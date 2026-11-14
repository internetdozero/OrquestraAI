//! Encryption at rest, re-exported from `ownhub-vault`.
//!
//! The sealer, the file-key lifecycle and the migration/backup sweep moved to
//! `crates/ownhub-vault/src/at_rest` (task #328). These re-exports keep
//! every `crate::at_rest::…` call site — the MCP catalog's sealed store in
//! particular — unchanged.

pub(crate) use ownhub_vault::at_rest::{
    open_stored, purge_migration_backup, seal_text, AAD_MCPS,
};
