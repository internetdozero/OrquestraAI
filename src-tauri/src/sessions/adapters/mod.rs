//! The [`SessionAdapter`] contract and the registry of all nine harnesses.
//!
//! Adapters are constructed with an explicit home directory so a test can
//! point them at a fixture tree. Listing is synchronous and read-only: the
//! commands may run on any thread, and a harness that is mid-conversation must
//! not be disturbed.

mod antigravity;
mod claude;
mod codex;
mod copilot;
mod freebuff;
mod freebuff_desktop;
pub mod fsops;
mod grok;
mod hermes;
mod kilo;
mod opencode;
mod sqlite_store;
#[cfg(test)]
mod tests;
pub mod time;
pub mod utils;

use std::collections::BTreeMap;
use std::path::Path;

use crate::sessions::types::UnifiedSession;

pub use utils::home;

/// Canonical harness ids, in the order the UI lists them.
pub const HARNESS_IDS: [&str; 9] = [
    "antigravity",
    "codex",
    "claude",
    "freebuff",
    "opencode",
    "kilo",
    "copilot",
    "hermes",
    "grok",
];

/// One harness's chat history, normalized to the unified listing.
pub trait SessionAdapter: Send + Sync {
    /// Canonical harness id from [`HARNESS_IDS`].
    fn harness_id(&self) -> &'static str;

    /// Is the harness installed? A missing directory is not an error.
    fn is_available(&self) -> bool;

    /// Every session this harness stores, newest or not — ordering is the
    /// caller's job once all harnesses are merged.
    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String>;

    /// Is a running agent holding this session open?
    ///
    /// Defaults to `false`: a harness with no lock or active-session concept
    /// has nothing to consult, and claiming otherwise would block every purge.
    fn is_session_active(&self, _session_id: &str) -> bool {
        false
    }

    /// Delete one session across every store the harness keeps it in, and
    /// return the bytes of files and folders removed. A missing session is not
    /// an error — purging twice must be harmless.
    fn delete_session(&self, session_id: &str) -> Result<u64, String>;

    /// Reclaim the pages a delete left in this harness's databases. Failing
    /// here is reported, never fatal: the rows are already gone.
    fn optimize_storage(&self) -> Result<(), String>;
}

/// All adapters, bound to one home directory.
pub fn registry(home: &Path) -> Vec<Box<dyn SessionAdapter>> {
    vec![
        Box::new(antigravity::Antigravity::new(home)),
        Box::new(codex::Codex::new(home)),
        Box::new(claude::Claude::new(home)),
        Box::new(freebuff::Freebuff::new(home)),
        Box::new(opencode::OpenCode::new(home)),
        Box::new(kilo::Kilo::new(home)),
        Box::new(copilot::Copilot::new(home)),
        Box::new(hermes::Hermes::new(home)),
        Box::new(grok::Grok::new(home)),
    ]
}

/// Merge every installed harness.
///
/// A harness that is missing, locked or corrupt contributes a warning and is
/// skipped: one broken store must never take the whole tab down. The returned
/// warnings are logged by the caller, never surfaced as a command error.
pub fn collect(home: &Path) -> (Vec<UnifiedSession>, Vec<String>) {
    let mut sessions = Vec::new();
    let mut warnings = Vec::new();

    for adapter in registry(home) {
        if !adapter.is_available() {
            continue;
        }

        match adapter.list_sessions() {
            Ok(mut found) => sessions.append(&mut found),
            Err(error) => warnings.push(format!("{}: {error}", adapter.harness_id())),
        }
    }

    (sessions, warnings)
}

/// Same as [`collect`], against the real home directory.
pub fn list_all() -> Result<Vec<UnifiedSession>, String> {
    let home = home()?;
    let (sessions, warnings) = collect(&home);

    for warning in &warnings {
        eprintln!("[orquestra] sessions: skipped {warning}");
    }

    Ok(sessions)
}

/// Registry keyed by harness id, for dispatching a deletion to the adapter
/// that owns the session.
pub fn adapters_by_id(home: &Path) -> BTreeMap<&'static str, Box<dyn SessionAdapter>> {
    registry(home)
        .into_iter()
        .map(|adapter| (adapter.harness_id(), adapter))
        .collect()
}
