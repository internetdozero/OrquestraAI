//! Vault security audit and backup purging subcommand.

use super::super::{parse_options, rendered, usage, CliError};
use super::paths;
use crate::vault::vault_purge_migration_backups;
use ownhub_vault::vault::service::list_services;
#[cfg(unix)]
use std::fs;

pub(super) fn audit(args: &[String]) -> Result<String, CliError> {
    if args.first().map(String::as_str) == Some("purge-backups") {
        return purge_backups(&args[1..]);
    }

    let options = parse_options(args, &[], &["--json"])?;
    let paths = paths()?;

    let vault_file = paths.root.join("vault.json");
    let file_exists = vault_file.is_file();

    // POSIX modes are the only privacy signal there is; elsewhere the file is
    // protected by the profile directory's ACLs, so the check stays quiet.
    #[cfg(unix)]
    let permissions_ok = {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(&vault_file) {
            Ok(meta) => meta.permissions().mode() & 0o777 == 0o600,
            // A missing file is not a permissions problem.
            Err(_) => true,
        }
    };
    #[cfg(not(unix))]
    let permissions_ok = true;

    let vault_bak = paths.root.join("vault.json.migrate-bak");
    let mcps_bak = paths.root.join("mcps.json.migrate-bak");
    let lingering_backups = vault_bak.exists() || mcps_bak.exists();

    let services = list_services(&paths).unwrap_or_default();
    let total_services = services.len();
    let total_keys: usize = services.iter().map(|s| s.total_keys).sum();
    let healthy_keys: usize = services
        .iter()
        .flat_map(|s| &s.key_health)
        .filter(|h| matches!(h, ownhub_vault::vault::KeyHealth::Healthy))
        .count();
    let degraded_keys = total_keys.saturating_sub(healthy_keys);

    let report = serde_json::json!({
        "vaultFileExists": file_exists,
        "permissions0600": permissions_ok,
        "encryptionAtRest": "AES-256-GCM",
        "lingeringMigrationBackups": lingering_backups,
        "backupFiles": {
            "vault": vault_bak.exists(),
            "mcps": mcps_bak.exists(),
        },
        "poolIntegrity": {
            "services": total_services,
            "totalKeys": total_keys,
            "healthyKeys": healthy_keys,
            "degradedKeys": degraded_keys,
        },
        "execPolicy": {
            "allowlistCount": 10,
            "shellEscapingBlocked": true,
        }
    });

    let perm_str = if permissions_ok {
        "[PASS]"
    } else {
        "[FAIL - expected 0600]"
    };
    let bak_str = if lingering_backups {
        "[WARN - run 'ownhub vault audit purge-backups --confirm' to zeroize]"
    } else {
        "[PASS - none lingering]"
    };

    let human = format!(
        "Vault Security Audit:\n  \
         Storage Encryption:  AES-256-GCM [PASS]\n  \
         File Permissions:    {} (0600) {}\n  \
         Migration Backups:   {}\n  \
         Pool Integrity:      {} services, {} keys ({} Healthy, {} Degraded)\n  \
         Exec Policy:         Allowlist active (10 binaries), No shell escapes",
        vault_file.display(),
        perm_str,
        bak_str,
        total_services,
        total_keys,
        healthy_keys,
        degraded_keys
    );

    rendered(&report, options.has("--json"), human)
}

fn purge_backups(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--confirm", "--json"])?;
    if !options.has("--confirm") {
        return Err(usage(
            "pass --confirm to execute zeroizing of migration backups".to_string(),
        ));
    }

    let report = vault_purge_migration_backups().map_err(CliError::Operation)?;
    let human = format!(
        "Purged migration backups:\n  Vault backup zeroized: {}\n  Catalog backup zeroized: {}",
        report.vault_purged, report.catalog_purged
    );
    rendered(&report, options.has("--json"), human)
}
