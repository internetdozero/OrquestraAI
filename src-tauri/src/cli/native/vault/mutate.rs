//! Vault mutation subcommands: set, add-key, remove-key, remove, enable, disable, toggle, set-enabled, set-primary, rotate.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use super::{parse_key_number, paths};
use super::super::{parse_options, rendered, usage, CliError};
use orquestra_vault::vault::service::{self, SaveServiceInput};

pub(super) fn set(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault set <id> [options]".to_string()))?;

    let options = parse_options(
        &args[1..],
        &[
            "--key",
            "--key-file",
            "--label",
            "--preset",
            "--desc",
            "--description",
            "--custom-header",
            "--custom-prefix",
            "--custom-env",
            "--meta",
            "--tag",
        ],
        &["--append", "--json"],
    )?;

    let secret_key = read_key_input(&options)?;
    let label = options.one("--label")?.unwrap_or_else(|| id.clone());
    let preset = options.one("--preset")?.unwrap_or_else(|| "custom".to_string());
    let description = options.one("--description")?.or(options.one("--desc")?);
    let custom_header = options.one("--custom-header")?;
    let custom_prefix = options.one("--custom-prefix")?;
    let custom_env = options.one("--custom-env")?;
    let tag = options.one("--tag")?;
    let append = options.has("--append");

    let mut meta = HashMap::new();
    for entry in options.repeated("--meta") {
        if let Some((k, v)) = entry.split_once('=') {
            meta.insert(k.trim().to_string(), v.trim().to_string());
        } else {
            return Err(usage(format!("invalid --meta format '{entry}', expected KEY=VAL")));
        }
    }

    let paths = paths()?;
    let existing_service = service::list_services(&paths).ok().and_then(|services| services.into_iter().find(|s| s.id == *id));

    let (keys, key_tags) = if append {
        let Some(existing) = existing_service else {
            return Err(CliError::Operation(format!("cannot --append: service '{id}' does not exist")));
        };
        let mut k = existing.masked_keys;
        k.push(secret_key.ok_or_else(|| usage("--key is required when appending a new key".to_string()))?);
        let mut t = existing.key_tags;
        t.push(tag);
        (k, Some(t))
    } else if let Some(key) = secret_key {
        (vec![key], Some(vec![tag]))
    } else if let Some(existing) = existing_service {
        (existing.masked_keys, Some(existing.key_tags))
    } else {
        return Err(usage("initial --key is required when registering a new service".to_string()));
    };

    let service = service::save_service(
        &paths,
        SaveServiceInput {
            id: id.clone(),
            label,
            preset,
            description,
            keys,
            custom_header,
            custom_prefix,
            custom_env,
            meta: if meta.is_empty() { None } else { Some(meta) },
            key_tags,
        },
    )
    .map_err(CliError::Operation)?;

    let human = format!("Saved vault service '{}' ({} keys registered).", service.label, service.total_keys);
    rendered(&service, options.has("--json"), human)
}

pub(super) fn add_key(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault add-key <id> --key <secret> [--tag <tag>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--key", "--key-file", "--tag"], &["--json"])?;
    let secret_key = read_key_input(&options)?
        .ok_or_else(|| usage("--key is required".to_string()))?;
    let tag = options.one("--tag")?;

    let paths = paths()?;
    let services = service::list_services(&paths).map_err(CliError::Operation)?;
    let existing = services
        .into_iter()
        .find(|s| s.id == *id)
        .ok_or_else(|| CliError::Operation(format!("unknown vault service: {id}")))?;

    let mut keys = existing.masked_keys;
    keys.push(secret_key);
    let mut tags = existing.key_tags;
    tags.push(tag);

    let updated = service::save_service(&paths, SaveServiceInput {
        id: existing.id,
        label: existing.label,
        preset: existing.preset,
        description: (!existing.description.is_empty()).then_some(existing.description),
        keys,
        custom_header: existing.custom_header,
        custom_prefix: existing.custom_prefix,
        custom_env: existing.custom_env,
        meta: if existing.meta.is_empty() { None } else { Some(existing.meta) },
        key_tags: Some(tags),
    }).map_err(CliError::Operation)?;

    let human = format!("Added key #{} to service '{}'.", updated.total_keys, updated.id);
    rendered(&updated, options.has("--json"), human)
}

pub(super) fn remove_key(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault remove-key <id> --key-number <N> [--yes] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--key-number"], &["--yes", "-y", "--json"])?;
    let key_num = options.required("--key-number")?;
    let index = parse_key_number(&key_num)? - 1;

    let paths = paths()?;
    let updated = service::remove_key(&paths, id, index).map_err(CliError::Operation)?;

    let human = format!("Removed key #{key_num} from service '{id}'. Total keys remaining: {}.", updated.total_keys);
    rendered(&updated, options.has("--json"), human)
}

pub(super) fn remove(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault remove <id> [--yes] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &[], &["--yes", "-y", "--json"])?;
    service::delete_service(&paths()?, id).map_err(CliError::Operation)?;

    let report = serde_json::json!({ "id": id, "deleted": true });
    let human = format!("Service '{id}' successfully removed from the vault.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn enable(args: &[String]) -> Result<String, CliError> {
    toggle_helper(args, Some(true), "enable")
}

pub(super) fn disable(args: &[String]) -> Result<String, CliError> {
    toggle_helper(args, Some(false), "disable")
}

pub(super) fn toggle(args: &[String]) -> Result<String, CliError> {
    toggle_helper(args, None, "toggle")
}

fn toggle_helper(args: &[String], desired: Option<bool>, verb: &str) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage(format!("usage: orquestra vault {verb} <id> [--json]")))?;
    let options = parse_options(&args[1..], &[], &["--json"])?;
    let paths = paths()?;
    let next_state = match desired {
        Some(state) => state,
        None => {
            let services = service::list_services(&paths).map_err(CliError::Operation)?;
            let current = services.into_iter().find(|s| s.id == *id).ok_or_else(|| CliError::Operation(format!("unknown service: {id}")))?;
            !current.enabled
        }
    };
    let svc = service::set_service_enabled(&paths, id, next_state).map_err(CliError::Operation)?;
    let human = format!("Service '{}' {}.", svc.label, if svc.enabled { "enabled" } else { "disabled" });
    rendered(&svc, options.has("--json"), human)
}

pub(super) fn set_enabled(args: &[String]) -> Result<String, CliError> {
    let (id, state, as_json) = match args {
        [id, state] => (id.as_str(), state.as_str(), false),
        [id, state, flag] if flag == "--json" => (id.as_str(), state.as_str(), true),
        _ => return Err(usage("usage: orquestra vault set-enabled <id> <on|off> [--json]".to_string())),
    };
    let enabled = match state {
        "on" | "true" | "enable" => true,
        "off" | "false" | "disable" => false,
        other => return Err(usage(format!("expected on|off, got {other}"))),
    };
    let svc = service::set_service_enabled(&paths()?, id, enabled).map_err(CliError::Operation)?;
    let human = format!("Service {} {}.", svc.label, if svc.enabled { "enabled" } else { "disabled" });
    rendered(&svc, as_json, human)
}

pub(super) fn set_primary(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault set-primary <id> --key-number <N> [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--key-number"], &["--json"])?;
    let key_num = options.required("--key-number")?;
    let index = parse_key_number(&key_num)? - 1;

    let svc = service::set_primary(&paths()?, id, index).map_err(CliError::Operation)?;
    let human = format!("Key #{key_num} is now the primary active key for service '{id}'.");
    rendered(&svc, options.has("--json"), human)
}

pub(super) fn rotate(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault rotate <id> [--next] [--key-number <N> --new-key <secret>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--key-number", "--new-key"], &["--next", "--json"])?;
    let paths = paths()?;
    let services = service::list_services(&paths).map_err(CliError::Operation)?;
    let existing = services.into_iter().find(|s| s.id == *id).ok_or_else(|| CliError::Operation(format!("unknown service: {id}")))?;

    if options.has("--next") {
        if existing.total_keys <= 1 {
            return Err(CliError::Operation(format!("service '{id}' only has 1 key; add more keys before rotating")));
        }
        let next_idx = (existing.active_key_index + 1) % existing.total_keys;
        let updated = service::set_primary(&paths, id, next_idx).map_err(CliError::Operation)?;
        let human = format!("Advanced service '{id}' primary key to #{} (round-robin).", next_idx + 1);
        return rendered(&updated, options.has("--json"), human);
    }

    if let (Some(num_str), Some(new_key)) = (options.one("--key-number")?, options.one("--new-key")?) {
        let index = parse_key_number(&num_str)? - 1;
        if index >= existing.total_keys {
            return Err(usage(format!("--key-number {num_str} out of range for service '{id}' (has {} keys)", existing.total_keys)));
        }
        let mut keys = existing.masked_keys;
        keys[index] = new_key;

        let updated = service::save_service(&paths, SaveServiceInput {
            id: existing.id,
            label: existing.label,
            preset: existing.preset,
            description: (!existing.description.is_empty()).then_some(existing.description),
            keys,
            custom_header: existing.custom_header,
            custom_prefix: existing.custom_prefix,
            custom_env: existing.custom_env,
            meta: if existing.meta.is_empty() { None } else { Some(existing.meta) },
            key_tags: Some(existing.key_tags),
        }).map_err(CliError::Operation)?;

        let human = format!("Replaced secret for key #{num_str} in service '{id}'.");
        return rendered(&updated, options.has("--json"), human);
    }

    Err(usage("usage: orquestra vault rotate <id> [--next] [--key-number <N> --new-key <secret>]".to_string()))
}

fn read_key_input(options: &super::super::Options) -> Result<Option<String>, CliError> {
    if let Some(file_path) = options.one("--key-file")? {
        let content = fs::read_to_string(&file_path)
            .map_err(|e| CliError::Operation(format!("failed to read --key-file '{file_path}': {e}")))?;
        return Ok(Some(content.trim().to_string()));
    }
    if let Some(raw) = options.one("--key")? {
        if raw == "-" {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)
                .map_err(|e| CliError::Operation(format!("failed to read key from stdin: {e}")))?;
            return Ok(Some(buf.trim().to_string()));
        }
        return Ok(Some(raw));
    }
    Ok(None)
}
