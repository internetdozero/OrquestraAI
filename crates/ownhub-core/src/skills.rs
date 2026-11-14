//! Shared layout constants for the skill vault.
//!
//! The projection and fusion layers both decide "is this a skill folder?" by
//! this one filename, so it lives beside the storage layout instead of in a
//! consumer.

/// The file that marks a directory as a skill.
pub const SKILL_FILE: &str = "SKILL.md";
