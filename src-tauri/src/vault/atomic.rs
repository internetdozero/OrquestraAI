//! Atomic vault writes: process-wide lock + unique tmp + rename.
//!
//! Extracted from `store.rs` so the storage module stays under the repo's
//! file-size cap. Every vault read-modify-write goes through
//! [`update_vault`]: lock, reload fresh, apply, save. Callers must NOT hold
//! a `VaultFile` across `.await` — pass secrets out, await, then mutate
//! here so concurrent writers are never clobbered.

use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, MutexGuard, OnceLock,
};

use crate::paths::OrquestraPaths;

use super::store::{load_vault, save_vault, VaultFile};

/// Process-wide mutex serializing every vault read-modify-write.
/// Tauri commands run concurrently on the async runtime; without this,
/// `load → HTTP (seconds) → save` in `vault_http_request` overwrites
/// concurrent `vault_save_service` mutations with its stale snapshot.
/// (Single-instance guard means one GUI process; cross-process `flock` is
/// intentionally out of scope — atomic tmp+rename keeps torn writes away.)
static VAULT_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn vault_lock() -> Result<MutexGuard<'static, ()>, String> {
    VAULT_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "vault lock poisoned".to_string())
}

/// Atomic read-modify-write: lock, reload fresh, apply, save — but skip the
/// disk write (rewrite + fsync) when `apply` changed nothing. Health pings
/// from `vault_http_request` re-record the same Healthy state on every
/// call; without this every request paid a full file rewrite + fsync for a
/// byte-identical vault. Comparison is on the serialized form of the same
/// in-memory instance, so equal bytes always mean equal content (a rehash
/// from an insert can only cause a harmless extra save, never a skip).
pub(super) fn update_vault<R>(
    paths: &OrquestraPaths,
    apply: impl FnOnce(&mut VaultFile) -> Result<R, String>,
) -> Result<R, String> {
    let _guard = vault_lock()?;
    let mut vault = load_vault(paths)?;
    let before = serde_json::to_string(&vault)
        .map_err(|error| format!("failed to serialize the credential vault: {error}"))?;
    let result = apply(&mut vault)?;
    let after = serde_json::to_string(&vault)
        .map_err(|error| format!("failed to serialize the credential vault: {error}"))?;
    if before != after {
        save_vault(paths, &vault)?;
    }
    Ok(result)
}

pub(super) fn write_private(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    // Unique tmp name: the old static `vault.json.tmp` collided when two
    // writers overlapped, so one rename ate the other's payload. Same
    // directory (same filesystem → atomic rename), unique per attempt.
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let thread_tag: String = format!("{:?}", std::thread::current().id())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let uniq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!(
        "json.tmp-{}-{}-{}-{}",
        std::process::id(),
        thread_tag,
        super::store::now_ms(),
        uniq
    ));
    {
        use std::io::Write;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        let mut file = options
            .open(&tmp)
            .map_err(|error| format!("failed to write {}: {error}", tmp.display()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", tmp.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", tmp.display()))?;
    }
    // A vault that cannot be locked down to 0o600 must fail closed: a
    // silent chmod miss would leave secrets readable by other users.
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to secure {}: {error}", tmp.display()))?;
    fs::rename(&tmp, path)
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to secure {}: {error}", path.display()))?;
    Ok(())
}
