//! Vault query subcommands: list, inspect, presets.

use super::super::{parse_options, rendered, usage, CliError};
use super::{parse_key_number, paths};
use orquestra_vault::vault::presets::list_presets;
use orquestra_vault::vault::service::{list_services, reveal_key};
use orquestra_vault::vault::VaultServicePublic;

pub(super) fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--preset", "--search"],
        &["--enabled-only", "--failing-only", "--json"],
    )?;

    let mut services = list_services(&paths()?).map_err(CliError::Operation)?;

    if let Some(preset) = options.one("--preset")? {
        services.retain(|s| s.preset.eq_ignore_ascii_case(&preset));
    }
    if options.has("--enabled-only") {
        services.retain(|s| s.enabled);
    }
    if options.has("--failing-only") {
        services.retain(|s| !s.health.eq_ignore_ascii_case("healthy"));
    }
    if let Some(term) = options.one("--search")? {
        let needle = term.to_lowercase();
        services.retain(|s| {
            s.id.to_lowercase().contains(&needle)
                || s.label.to_lowercase().contains(&needle)
                || s.description.to_lowercase().contains(&needle)
        });
    }

    let human = format_services_table(&services);
    rendered(&services, options.has("--json"), human)
}

pub(super) fn inspect(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| {
            usage(
                "usage: orquestra vault inspect <id> [--reveal] [--key-number <N>] [--json]"
                    .to_string(),
            )
        })?;

    let options = parse_options(&args[1..], &["--key-number"], &["--reveal", "--json"])?;
    let reveal = options.has("--reveal");
    let key_num = options
        .one("--key-number")?
        .map(|v| parse_key_number(&v))
        .transpose()?;

    let paths = paths()?;
    let services = list_services(&paths).map_err(CliError::Operation)?;
    let service = services
        .into_iter()
        .find(|s| s.id == *id)
        .ok_or_else(|| CliError::Operation(format!("unknown vault service: {id}")))?;

    let mut revealed_secrets: Vec<Option<String>> = vec![None; service.total_keys];
    if reveal {
        if let Some(num) = key_num {
            let secret = reveal_key(&paths, id, num - 1).map_err(CliError::Operation)?;
            if num - 1 < revealed_secrets.len() {
                revealed_secrets[num - 1] = Some(secret);
            }
        } else {
            for (i, secret_slot) in revealed_secrets.iter_mut().enumerate() {
                if let Ok(sec) = reveal_key(&paths, id, i) {
                    *secret_slot = Some(sec);
                }
            }
        }
    }

    let report = serde_json::json!({
        "service": service,
        "revealed": reveal,
        "secrets": if reveal { Some(&revealed_secrets) } else { None },
    });

    let human = format_inspect_human(&service, &revealed_secrets);
    rendered(&report, options.has("--json"), human)
}

pub(super) fn presets(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let preset_list = list_presets();
    let human = format_presets_table(&preset_list);
    rendered(&preset_list, options.has("--json"), human)
}

fn format_services_table(services: &[VaultServicePublic]) -> String {
    if services.is_empty() {
        return "No credential services found.".to_string();
    }
    let mut lines = Vec::with_capacity(services.len() + 1);
    lines.push(format!(
        "{:<14} {:<20} {:<12} {:<10} {:<10} {:<6} {}",
        "SERVICE ID", "LABEL", "PRESET", "STATUS", "HEALTH", "KEYS", "ACTIVE"
    ));
    for s in services {
        let status = if s.enabled { "enabled" } else { "disabled" };
        let active = format!("#{}", s.active_key_index + 1);
        lines.push(format!(
            "{:<14} {:<20} {:<12} {:<10} {:<10} {:<6} {}",
            s.id, s.label, s.preset, status, s.health, s.total_keys, active
        ));
    }
    lines.join("\n")
}

fn format_inspect_human(service: &VaultServicePublic, revealed: &[Option<String>]) -> String {
    let state = if service.enabled {
        "Enabled (Kill-switch: OFF)"
    } else {
        "Disabled (Kill-switch: ON)"
    };
    let mut meta_lines = Vec::new();
    if service.meta.is_empty() {
        meta_lines.push("  (none)".to_string());
    } else {
        for (k, v) in &service.meta {
            meta_lines.push(format!("  {k}: {v}"));
        }
    }

    let mut pool_lines = Vec::new();
    for i in 0..service.total_keys {
        let tag = service
            .key_tags
            .get(i)
            .and_then(|t| t.as_deref())
            .map_or(String::new(), |t| format!("[{t}] "));
        let key_str = if let Some(Some(secret)) = revealed.get(i) {
            secret.clone()
        } else {
            service
                .masked_keys
                .get(i)
                .cloned()
                .unwrap_or_else(|| "••••••••".to_string())
        };
        let health = service
            .key_health
            .get(i)
            .map(|h| format!("{h:?}"))
            .unwrap_or_else(|| "Healthy".to_string());
        let primary_badge = if i == service.active_key_index {
            " [PRIMARY]"
        } else {
            ""
        };
        pool_lines.push(format!(
            "  #{:<2} {}{:<24} ({}){}",
            i + 1,
            tag,
            key_str,
            health,
            primary_badge
        ));
    }

    format!(
        "Service:     {}\n\
         Label:       {}\n\
         Preset:      {}\n\
         State:       {}\n\
         Health:      {}\n\
         Description: {}\n\
         Metadata:\n{}\n\n\
         Key Pool ({} keys):\n{}",
        service.id,
        service.label,
        service.preset,
        state,
        service.health,
        if service.description.is_empty() {
            "(none)"
        } else {
            &service.description
        },
        meta_lines.join("\n"),
        service.total_keys,
        pool_lines.join("\n")
    )
}

fn format_presets_table(presets: &[orquestra_vault::vault::presets::PresetSpec]) -> String {
    let mut lines = Vec::with_capacity(presets.len() + 1);
    lines.push(format!(
        "{:<14} {:<16} {:<16} {:<22} {}",
        "PRESET", "LABEL", "AUTH HEADER", "ENV VAR", "META FIELDS"
    ));
    for p in presets {
        let meta_str = if p.meta_fields.is_empty() {
            "-".to_string()
        } else {
            p.meta_fields
                .iter()
                .map(|m| m.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!(
            "{:<14} {:<16} {:<16} {:<22} {}",
            p.id, p.label, p.header, p.env_var, meta_str
        ));
    }
    lines.join("\n")
}
