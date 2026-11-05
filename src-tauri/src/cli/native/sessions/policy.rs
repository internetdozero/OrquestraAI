//! Retention policy inspection and mutation subcommand.

use super::super::{parse_options, parse_u32, rendered, usage, CliError};
use crate::sessions::{get_retention_policy, save_retention_policy};
use crate::sessions::types::RetentionPolicy;

pub(super) fn policy(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("show") | None => show(&args[args.first().map_or(0, |_| 1)..]),
        Some("set") => set(&args[1..]),
        _ => Err(usage("usage: orquestra sessions policy <show|set> [options]".to_string())),
    }
}

fn show(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let p = get_retention_policy().map_err(CliError::Operation)?;
    let human = format_policy_human(&p);
    rendered(&p, options.has("--json"), human)
}

fn set(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--default-ttl", "--auto-purge", "--interval", "--override"],
        &["--json"],
    )?;

    let mut current = get_retention_policy().map_err(CliError::Operation)?;

    if let Some(ttl) = parse_u32(&options, "--default-ttl")? {
        current.default_ttl_days = ttl;
    }

    if let Some(auto) = options.one("--auto-purge")? {
        current.auto_purge_enabled = match auto.as_str() {
            "on" | "true" | "enable" => true,
            "off" | "false" | "disable" => false,
            other => return Err(usage(format!("expected on|off for --auto-purge, got '{other}'"))),
        };
    }

    if let Some(interval) = parse_u32(&options, "--interval")? {
        current.auto_purge_interval_hours = interval;
    }

    for item in options.repeated("--override") {
        if let Some((harness, days_str)) = item.split_once('=') {
            let days = days_str
                .parse::<u32>()
                .map_err(|_| usage(format!("invalid TTL days in override '{item}'")))?;
            current.harness_overrides.insert(harness.trim().to_string(), days);
        } else {
            return Err(usage(format!("invalid --override format '{item}', expected harness=days")));
        }
    }

    save_retention_policy(current.clone()).map_err(CliError::Operation)?;

    let human = format!("Updated retention policy:\n{}", format_policy_human(&current));
    rendered(&current, options.has("--json"), human)
}

fn format_policy_human(policy: &RetentionPolicy) -> String {
    let mut overrides_lines = Vec::new();
    if policy.harness_overrides.is_empty() {
        overrides_lines.push("  (None)".to_string());
    } else {
        for (h, days) in &policy.harness_overrides {
            overrides_lines.push(format!("  • {h}: {days} days"));
        }
    }

    format!(
        "Retention Policy:\n  \
         Default TTL:       {} days\n  \
         Auto-Purge Active: {}\n  \
         Interval:          {} hours\n  \
         Last Run:          {}\n  \
         Harness Overrides:\n{}",
        policy.default_ttl_days,
        if policy.auto_purge_enabled { "yes" } else { "no" },
        policy.auto_purge_interval_hours,
        policy.last_run.as_deref().unwrap_or("never"),
        overrides_lines.join("\n")
    )
}
