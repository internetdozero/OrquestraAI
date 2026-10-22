//! `orquestra-vault`: the credential vault and its cryptographic core.
//!
//! Two layers live here, both free of the Tauri runtime:
//!
//! - [`at_rest`] seals the secret-bearing JSON files (`vault.json`,
//!   `mcps.json`) with AES-256-GCM under a per-install file key, and owns the
//!   migration/unseal/backup lifecycle around it.
//! - [`vault`] is the blind credential store: services and key pools, the
//!   SSRF-gated blind HTTP proxy, and the shell-free exec proxy. Secrets are
//!   injected in memory and never returned to the caller.
//!
//! The desktop shell keeps only the `#[tauri::command]` wrappers and calls
//! into the operations re-exported from [`vault`].

mod logging;

pub mod at_rest;
pub mod vault;
