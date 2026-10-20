//! Slugs for custom preset ids. A name becomes a path-safe id that never
//! shadows a built-in or another custom profile.

use super::builtins;
use super::store::PresetFile;

/// `Game Dev` → `game-dev`; runs of punctuation collapse to one dash and a
/// name with no usable characters becomes `preset`.
pub(super) fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;

    for character in name.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            if separator_pending && !slug.is_empty() {
                slug.push('-');
            }
            separator_pending = false;
            slug.push(character);
        } else {
            separator_pending = true;
        }
    }

    if slug.is_empty() {
        "preset".to_string()
    } else {
        slug
    }
}

/// A slug that is free to use, never shadowing a built-in or another custom.
pub(super) fn unique_id(name: &str, file: &PresetFile) -> String {
    let base = slugify(name);
    let taken = |candidate: &str| {
        builtins::all().iter().any(|preset| preset.id == candidate)
            || file.custom.iter().any(|preset| preset.id == candidate)
    };

    if !taken(&base) {
        return base;
    }

    for suffix in 2..10_000 {
        let candidate = format!("{base}-{suffix}");
        if !taken(&candidate) {
            return candidate;
        }
    }

    format!("{base}-{}", std::process::id())
}
