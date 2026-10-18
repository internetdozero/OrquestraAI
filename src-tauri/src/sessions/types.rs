//! Wire types for the unified Sessions tab.
//!
//! One `UnifiedSession` is one conversation as it exists on disk in one
//! harness. OrquestraAI never rewrites a harness schema to store its own
//! metadata: pin state and retention live in `~/.orquestra` (see `policy`),
//! and `UnifiedSession::is_pinned` is decorated at read time.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Longest title the UI receives. Prompts are trimmed on the core so the grid
/// never has to slice a multi-megabyte first message.
pub const TITLE_LIMIT: usize = 120;

/// One conversation from any harness, normalized to the fields the tab shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSession {
    /// Harness-native session id, unique together with `harness`.
    pub id: String,
    /// Canonical harness id: `antigravity`, `codex`, `claude`, `freebuff`,
    /// `opencode`, `kilo`, `copilot`, `hermes`, `grok`.
    pub harness: String,
    /// Session summary or first prompt, trimmed to [`TITLE_LIMIT`].
    pub title: String,
    /// Workspace / working directory when the harness recorded one.
    pub project_path: Option<String>,
    /// ISO 8601 UTC timestamp of the last activity.
    pub updated_at: String,
    /// ISO 8601 UTC timestamp of the first activity, when known.
    pub created_at: Option<String>,
    /// User turns in the session, or `0` when the harness does not expose it
    /// cheaply (listing must stay a read-only scan, not a full replay).
    pub turns_count: usize,
    /// Estimated bytes on disk: session files plus content rows.
    pub size_bytes: u64,
    /// Decorated from the pin list in `~/.orquestra/sessions_policy.json`.
    pub is_pinned: bool,
    /// Optional operator label for a pinned session.
    pub pin_label: Option<String>,
}

impl UnifiedSession {
    /// Skeleton for an adapter: everything except the pin decoration, which
    /// the policy layer applies once for all harnesses.
    pub fn new(harness: &str, id: impl Into<String>, title: String) -> Self {
        Self {
            id: id.into(),
            harness: harness.to_string(),
            title,
            project_path: None,
            updated_at: String::new(),
            created_at: None,
            turns_count: 0,
            size_bytes: 0,
            is_pinned: false,
            pin_label: None,
        }
    }
}

/// Filter the Sessions tab (and, from Phase 2, purge) applies in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionFilter {
    /// Restrict to one canonical harness id.
    pub harness: Option<String>,
    /// Case-insensitive substring matched against title and project path.
    pub search: Option<String>,
    /// Only sessions the operator pinned.
    pub pinned_only: bool,
    /// Only sessions untouched for at least this many days (purge candidates).
    pub older_than_days: Option<u32>,
}

/// Retention rules for the auto-purge scheduler, stored next to the pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    /// Default age, in days, after which a session becomes a purge candidate.
    pub default_ttl_days: u32,
    /// Per-harness overrides keyed by canonical harness id.
    pub harness_overrides: HashMap<String, u32>,
    /// Opt-in: nothing is ever deleted on a schedule until the operator asks.
    pub auto_purge_enabled: bool,
    /// How often the scheduler may run.
    pub auto_purge_interval_hours: u32,
    /// ISO 8601 timestamp of the last completed purge.
    pub last_run: Option<String>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            default_ttl_days: 15,
            harness_overrides: HashMap::new(),
            auto_purge_enabled: false,
            auto_purge_interval_hours: 24,
            last_run: None,
        }
    }
}

impl RetentionPolicy {
    /// Days a session of `harness` is kept, preferring its override.
    pub fn ttl_days_for(&self, harness: &str) -> u32 {
        self.harness_overrides
            .get(harness)
            .copied()
            .unwrap_or(self.default_ttl_days)
    }
}

/// One protected session. Stored only in `~/.orquestra`, never in a harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedSession {
    pub harness: String,
    pub session_id: String,
    /// ISO 8601 UTC timestamp of the pin action.
    pub pinned_at: String,
    pub label: Option<String>,
}

/// Which sessions a purge may touch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PurgeFilter {
    /// Restrict to one canonical harness id.
    pub harness: Option<String>,
    /// Only sessions untouched for at least this many days. `None` means every
    /// session the other selectors already match, including recent ones.
    pub older_than_days: Option<u32>,
    /// Surgical deletes: exact session ids, intersected with the other fields.
    pub session_ids: Option<Vec<String>>,
    /// Never delete a pinned session while this is `true` (the default).
    pub ignore_pinned: bool,
}

impl Default for PurgeFilter {
    fn default() -> Self {
        Self {
            harness: None,
            older_than_days: None,
            session_ids: None,
            // Pinning is a promise: it only breaks when the operator says so.
            ignore_pinned: true,
        }
    }
}

/// What a purge *would* do. Produced without touching anything on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeEstimate {
    pub sessions_count: usize,
    pub reclaimed_bytes: u64,
    /// Canonical harness ids the purge would touch, sorted.
    pub affected_harnesses: Vec<String>,
    /// Protected by a pin and therefore left alone.
    pub skipped_pinned_count: usize,
    /// Held open or locked by a running agent and therefore left alone.
    pub skipped_active_count: usize,
}

/// Outcome of a purge run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    pub deleted_count: usize,
    /// Bytes of session files and folders removed. SQLite page reclamation is
    /// not part of this figure: it happens during `VACUUM`, after the rows are
    /// gone, and is verified separately.
    pub reclaimed_bytes: u64,
    pub failed_count: usize,
    /// One human-readable line per failure, including a failed `VACUUM`, which
    /// does not count as a failed deletion.
    pub errors: Vec<String>,
}
