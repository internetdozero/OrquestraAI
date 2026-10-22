//! Encryption at rest, re-exported from `orquestra-vault`.
//!
//! The sealer, the file-key lifecycle and the migration/backup sweep moved to
//! `crates/orquestra-vault/src/at_rest` (task #328). These re-exports keep
//! every `crate::at_rest::…` call site — the MCP catalog's sealed store in
//! particular — unchanged.

pub(crate) use orquestra_vault::at_rest::{
    open_stored, purge_migration_backup, seal_text, AAD_MCPS,
};
