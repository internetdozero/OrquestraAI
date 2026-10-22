//! Atomic persistence for pins and retention rules in
//! `~/.orquestra/sessions_policy.json`.
//!
//! This is deliberately the *only* place OrquestraAI stores session metadata.
//! Pinning a conversation must never rewrite a harness's own SQLite schema, so
//! the policy file is a plain JSON sidecar: deleting it forgets the pins and
//! leaves every harness byte-identical.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::types::{PinnedSession, RetentionPolicy};
use crate::paths::{create_dir, OrquestraPaths};

/// Sidecar file inside `~/.orquestra`.
const POLICY_FILE: &str = "sessions_policy.json";

/// Largest TTL the UI can ask for; anything bigger is a unit mistake.
const MAX_TTL_DAYS: u32 = 3650;

/// On-disk shape. Both halves default, so a hand-edited or truncated file that
/// still parses as JSON never loses the half it did carry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PolicyFile {
    pub(crate) pinned_sessions: Vec<PinnedSession>,
    pub(crate) retention: RetentionPolicy,
}

// ---------------------------------------------------------------------------
// Public API used by the IPC commands
// ---------------------------------------------------------------------------

/// Current retention policy, falling back to defaults on first run.
pub fn get_policy() -> Result<RetentionPolicy, String> {
    Ok(load_from(&root()?)?.retention)
}

/// Replace the retention policy, keeping the existing pin list untouched.
pub fn save_policy(policy: RetentionPolicy) -> Result<(), String> {
    let root = root()?;
    let mut file = load_from(&root)?;
    file.retention = sanitized(policy);
    save_to(&root, &file)
}

/// Pin or unpin one session. Returns the state *after* the toggle: `true` when
/// the session is now protected, `false` when the pin was removed.
pub fn toggle_pin(harness: &str, session_id: &str, label: Option<String>) -> Result<bool, String> {
    toggle_pin_at(&root()?, harness, session_id, label, now_iso8601())
}

/// Every pinned session, used to decorate a listing.
pub(crate) fn pins() -> Result<Vec<PinnedSession>, String> {
    Ok(load_from(&root()?)?.pinned_sessions)
}

// ---------------------------------------------------------------------------
// Testable core (explicit root, explicit clock)
// ---------------------------------------------------------------------------

pub(crate) fn policy_path(root: &Path) -> PathBuf {
    root.join(POLICY_FILE)
}

/// Read the sidecar. A missing file is the normal first-run state and yields
/// defaults rather than an error; an unreadable *present* file is an error so
/// the caller can surface it instead of silently dropping the operator's pins.
pub(crate) fn load_from(root: &Path) -> Result<PolicyFile, String> {
    let path = policy_path(root);
    if fs::symlink_metadata(&path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to read {}: not a regular file",
            path.display()
        ));
    }
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PolicyFile::default())
        }
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };

    serde_json::from_str(&raw)
        .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))
}

/// Write the sidecar atomically: full contents to a sibling temporary file,
/// then rename over the target so a crash never leaves a half-written policy.
pub(crate) fn save_to(root: &Path, file: &PolicyFile) -> Result<(), String> {
    create_dir(root)?;
    let path = policy_path(root);
    if fs::symlink_metadata(&path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to replace {}: not a regular file",
            path.display()
        ));
    }
    let temporary = root.join(format!(
        ".{POLICY_FILE}.tmp-{}",
        crate::paths::unique_tmp_suffix()
    ));

    let mut content = serde_json::to_string_pretty(file)
        .map_err(|error| format!("failed to serialize the session policy: {error}"))?;
    content.push('\n');

    let mut handle = fs::File::create(&temporary)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    let written = handle
        .write_all(content.as_bytes())
        .and_then(|()| handle.sync_all());
    drop(handle);

    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(format!("failed to write {}: {error}", temporary.display()));
    }

    fs::rename(&temporary, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })
}

pub(crate) fn toggle_pin_at(
    root: &Path,
    harness: &str,
    session_id: &str,
    label: Option<String>,
    pinned_at: String,
) -> Result<bool, String> {
    let mut file = load_from(root)?;
    let existing = file
        .pinned_sessions
        .iter()
        .position(|pin| pin.harness == harness && pin.session_id == session_id);

    if let Some(index) = existing {
        file.pinned_sessions.remove(index);
        save_to(root, &file)?;
        return Ok(false);
    }

    file.pinned_sessions.push(PinnedSession {
        harness: harness.to_string(),
        session_id: session_id.to_string(),
        pinned_at,
        label: label
            .map(|label| label.trim().to_string())
            .filter(|label| !label.is_empty()),
    });
    save_to(root, &file)?;
    Ok(true)
}

/// Record that a purge finished, which is what the scheduler uses to decide
/// whether the interval has elapsed. A missing sidecar is created, and the pin
/// list survives untouched.
pub(crate) fn mark_last_run_at(root: &Path, at: String) -> Result<(), String> {
    let mut file = load_from(root)?;
    file.retention.last_run = Some(at);
    save_to(root, &file)
}

/// Record a run against the real `~/.orquestra` root.
pub fn mark_last_run(at: String) -> Result<(), String> {
    mark_last_run_at(&root()?, at)
}

/// Keep the stored policy inside the range the scheduler can honour.
pub(crate) fn sanitized(policy: RetentionPolicy) -> RetentionPolicy {
    RetentionPolicy {
        default_ttl_days: policy.default_ttl_days.min(MAX_TTL_DAYS),
        harness_overrides: policy
            .harness_overrides
            .into_iter()
            .map(|(harness, days)| (harness, days.min(MAX_TTL_DAYS)))
            .collect(),
        auto_purge_enabled: policy.auto_purge_enabled,
        auto_purge_interval_hours: policy.auto_purge_interval_hours.clamp(1, 720),
        last_run: policy.last_run,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn root() -> Result<PathBuf, String> {
    Ok(OrquestraPaths::resolve()?.root)
}

pub(crate) fn now_iso8601() -> String {
    super::adapters::utils::iso8601_from_secs(super::adapters::utils::now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    #[test]
    fn toggle_pin_does_not_follow_a_policy_symlink() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-policy-link-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.json");
        let original = "{\n  \"pinnedSessions\": [{\n    \"harness\": \"codex\",\n    \"sessionId\": \"ssh-private\",\n    \"pinnedAt\": \"2026-10-18T06:00:00Z\"\n  }]\n}\n";
        fs::write(&outside, original).unwrap();
        let link = root.join(POLICY_FILE);
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let result = toggle_pin_at(
            &root,
            "claude",
            "s1",
            None,
            "2026-10-18T06:00:00Z".to_string(),
        );
        assert!(
            result.is_err(),
            "pin must not read through the policy symlink: {result:?}"
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the policy symlink must stay"
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), original);
        let _ = fs::remove_dir_all(&root);
    }
}
