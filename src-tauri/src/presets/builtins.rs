//! The mission presets shipped with the application.
//!
//! Each built-in is a curated answer to a common working mode. The ids are
//! stable because they are what the terminal flag (`--preset stealth`) and the
//! agent tool call use; the names and icons are presentation only.

use super::Preset;

/// Every built-in profile, in the order the preset bar offers them.
pub(super) fn all() -> Vec<Preset> {
    vec![
        selective(
            "stealth",
            "Stealth",
            "shield",
            "Zero-bloat profile: one lightweight skill plus the core MCPs, so a \
             rate-limited model keeps its whole context window.",
            &["rtk"],
            &["headroom", "idea-vault", "orquestra-core"],
        ),
        selective(
            "rust-dev",
            "Rust Dev",
            "cpu",
            "Systems and low-level work: repository, module and knowledge \
             servers without the frontend or design tooling.",
            &["rtk", "modular-start", "github"],
            &["github", "rea", "idea-vault", "orquestra-core"],
        ),
        selective(
            "web-fullstack",
            "Web Fullstack",
            "globe",
            "Frontend and cloud work: UI generation, browser automation, design \
             assets and the deployment edge.",
            &["rtk", "github", "generative_ui"],
            &["puppeteer", "canva", "github", "idea-vault", "cloudflare"],
        ),
        Preset {
            id: "full-power".to_string(),
            name: "Full Power".to_string(),
            description: "Every skill in the vault and every MCP server in the \
                          catalog: nothing held back."
                .to_string(),
            icon: "zap".to_string(),
            skills: Vec::new(),
            mcps: Vec::new(),
            all_skills: true,
            all_mcps: true,
            builtin: true,
            active: false,
        },
    ]
}

/// A built-in that lists its members explicitly.
fn selective(
    id: &str,
    name: &str,
    icon: &str,
    description: &str,
    skills: &[&str],
    mcps: &[&str],
) -> Preset {
    Preset {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        icon: icon.to_string(),
        skills: skills.iter().map(|value| value.to_string()).collect(),
        mcps: mcps.iter().map(|value| value.to_string()).collect(),
        all_skills: false,
        all_mcps: false,
        builtin: true,
        active: false,
    }
}
