//! Encryption at rest for the secret-bearing files.
//!
//! `vault.json` and `mcps.json` carry API tokens, so they are sealed with
//! AES-256-GCM under a per-install file key (see [`key`]: OS keyring first,
//! `0600` key-file fallback on headless hosts). Every secret file is created
//! with mode `0600` — no write-then-chmod window — and writes stay atomic
//! (unique tmp + rename).
//!
//! Envelope versions: v1 (alpha.5, no AAD) still opens; every seal writes
//! v2, which binds the payload domain (`vault.v1` / `mcps.v1`) as AES-GCM
//! Additional Authenticated Data, so a `vault.json` envelope pasted into
//! `mcps.json` fails closed. Callers pass their domain and use
//! [`open_stored`], which migrates legacy plaintext (verify-then-unlink),
//! upgrades v1 → v2, and promotes a fallback key into the keyring.
//!
//! Once an install completes the encryption migration, [`key`]'s migrated
//! flag is set and plaintext secret files are refused instead of
//! re-sealed (downgrade protection). New migrations leave no plaintext
//! behind: the `<file>.migrate-bak` backup is unlinked once the sealed
//! file verifies, or overwritten sealed when verification fails. Plaintext
//! backups left by alpha.5 are swept by `startup_cleanup` once the sealed
//! file verifies — the operator still rotates the exposed keys (see
//! `CHANGELOG.md`) and purges sealed leftovers explicitly.
//!
//! The work is split by job: `seal` writes, `unseal` reads and verifies,
//! `migrate` handles legacy plaintext and self-healing, `backup` owns the
//! `.migrate-bak` lifecycle, `io` holds the file primitives and `key`
//! resolves the file key.

mod backup;
mod envelope;
mod io;
mod key;
mod migrate;
mod seal;
#[cfg(test)]
mod tests;
mod unseal;

pub use backup::purge_migration_backup;
pub use envelope::{AAD_MCPS, AAD_VAULT};
pub use seal::seal_text;
pub use unseal::open_stored;

// Test-only surface: production code reaches the flag via `key::is_migrated`.
#[cfg(test)]
pub(crate) use key::is_migrated;
