# Blind Vault — At-Rest Security & Migration Backups

## Key model

`vault.json` and `mcps.json` carry API tokens sealed with AES-256-GCM under
a per-install file key. The key lives in the OS keyring (Secret Service on
Linux, Keychain on macOS, Credential Manager on Windows); headless hosts
fall back to a `0600` key file beside the secrets with a loud once-per-process
warning. Files stay `0600`; integration tests never touch the real keyring
(`ORQUESTRA_KEYRING_DISABLE=1` plus a temp-root guard in `at_rest::key`).

## Why `.migrate-bak` files exist

The alpha.5 encryption rollout migrated plaintext files in place: the
original bytes were kept as `<file>.migrate-bak` so a failed migration never
loses data. The same backups power self-healing — if a sealed file stops
opening under the current key, the loaders recover from the backup and
re-seal under the active key instead of bricking unlock.

## Purging plaintext backups

Backups are kept until the operator explicitly purges them: auto-deleting on
first write would strand anyone whose new key cannot open the sealed files.

1. Confirm every service loads normally (Credentials screen, MCP list).
2. Open Credentials → **Purge plaintext backups**, confirm the dialog.
3. The backend (`vault_purge_migration_backups`) verifies each sealed file
   still opens, then overwrites each backup with zeroes, syncs, and unlinks
   it. The toast reports per file (`vault`, `catalog`); a missing backup is
   not an error.

The purge refuses while a sealed file does not open, refuses symlinks, and
never touches harness configs.

## Rotate pre-alpha.5 provider keys

Secrets stored before `v0.1.0-alpha.5` may have existed as plaintext on
disk. After purging, rotate those provider keys in their dashboards
(GitHub, OpenAI, Anthropic, Cloudflare, …) and replace them via the vault
key-rotation action. Zeroization is best-effort on SSDs and copy-on-write
filesystems (the flash translation layer may retain stale blocks outside
the filesystem's reach) — rotation is what eliminates the residual risk.
