//! The mission presets shipped with the application.
//!
//! New users with empty vaults see only [`all`]: the Full Power profile takes
//! the whole known set, so there are never warnings for missing skills/MCPs.
//! The id is stable because it is what the terminal flag
//! (`--preset full-power`) and the agent tool call use; the name and icon are
//! presentation only.

use super::Preset;

/// Every built-in profile, in the order the preset bar offers them.
pub(super) fn all() -> Vec<Preset> {
    vec![Preset {
        id: "full-power".to_string(),
        name: "Full Power".to_string(),
        description:
            "Every skill in the vault and every MCP server in the catalog: nothing held back."
                .to_string(),
        icon: "zap".to_string(),
        skills: Vec::new(),
        mcps: Vec::new(),
        harness_skills: Default::default(),
        harness_mcps: Default::default(),
        all_skills: true,
        all_mcps: true,
        builtin: true,
        active: false,
    }]
}
