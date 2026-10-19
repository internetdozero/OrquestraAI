//! Memoized `SKILL.md` parses for the vault listing.
//!
//! Split out of `vault.rs` so neither file grows the monolith. Every
//! `list_skills` call used to re-read and re-parse every skill file plus
//! re-run the token estimate and category rules — ~60 ms for a 400-skill
//! vault on every scope switch — while the bytes almost never change between
//! calls. The cache keys each skill directory on its `SKILL.md` fingerprint
//! (mtime nanos + size): any content edit bumps mtime, so a fingerprint
//! match proves the cached parse is still fresh.
//!
//! Deliberately NOT cached: projection state (scope-dependent, stays live)
//! and the security walk (no stat-only fingerprint can prove a deep file
//! tree unchanged, so it runs live and is always correct).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use super::vault::{
    categorize, estimate_tokens, normalize_description, split_frontmatter, Frontmatter,
};
use crate::official::is_official_skill;

/// Everything `list_from` derives from `SKILL.md` itself.
#[derive(Debug, Clone)]
pub(super) struct CachedContent {
    fingerprint: (u128, u64),
    pub(super) name: String,
    pub(super) description: String,
    pub(super) category: String,
    pub(super) tokens: usize,
    pub(super) official: bool,
}

static CONTENT_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedContent>>> = OnceLock::new();

fn content_cache() -> &'static Mutex<HashMap<PathBuf, CachedContent>> {
    CONTENT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn locked_cache() -> std::sync::MutexGuard<'static, HashMap<PathBuf, CachedContent>> {
    content_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// (mtime nanos, size) of `SKILL.md`. `None` when the file cannot be stated,
/// which reads as absent — the caller skips the skill, like before.
fn fingerprint(skill_file: &Path) -> Option<(u128, u64)> {
    let metadata = std::fs::metadata(skill_file).ok()?;
    let mtime = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((mtime, metadata.len()))
}

fn parse_content(id: &str, content: &str, fingerprint: (u128, u64)) -> CachedContent {
    let (frontmatter, body) = split_frontmatter(content);
    let metadata = frontmatter
        .and_then(|yaml| serde_yaml::from_str::<Frontmatter>(yaml).ok())
        .unwrap_or_default();

    let name = metadata
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| id.to_string());

    let description = metadata
        .description
        .as_deref()
        .map(normalize_description)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let fallback = super::vault::first_paragraph(body);
            if fallback.is_empty() {
                "No description provided.".to_string()
            } else {
                fallback
            }
        });

    CachedContent {
        fingerprint,
        category: categorize(id, &name, &description).to_string(),
        tokens: estimate_tokens(content),
        official: is_official_skill(id),
        name,
        description,
    }
}

/// Parsed content for one skill: cache hit when `SKILL.md` is untouched
/// since the last parse, fresh parse otherwise. `None` when the file is
/// unreadable.
pub(super) fn cached_content(
    directory: &Path,
    id: &str,
    skill_file: &Path,
) -> Option<CachedContent> {
    let fingerprint = fingerprint(skill_file)?;
    if let Some(hit) = locked_cache()
        .get(directory)
        .filter(|cached| cached.fingerprint == fingerprint)
    {
        return Some(hit.clone());
    }
    let content = std::fs::read_to_string(skill_file).ok()?;
    let parsed = parse_content(id, &content, fingerprint);
    locked_cache().insert(directory.to_path_buf(), parsed.clone());
    Some(parsed)
}

/// Drop entries for skills that left the vault, so deleted skills and
/// ad-hoc test vaults never pin memory.
pub(super) fn prune_content_cache(keep: &HashSet<PathBuf>) {
    locked_cache().retain(|path, _| keep.contains(path));
}
