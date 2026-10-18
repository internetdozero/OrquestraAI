//! Wire types for the native Idea Vault module.
//!
//! Shapes mirror the Python `idea-vault` MCP (`record_idea` /
//! `post_board_note`) so agents keep working while OrquestraAI reads and
//! mutates the same SQLite rows natively. Everything serializes camelCase,
//! like the rest of the IPC surface.

use serde::{Deserialize, Serialize};

/// Closed vocabularies, exactly as the MCP validates them.
pub const CATEGORIES: [&str; 5] = ["feature", "architecture", "bug", "ux", "idea"];
pub const PRIORITIES: [&str; 4] = ["low", "medium", "high", "critical"];
pub const STATUSES: [&str; 4] = ["open", "in_progress", "completed", "archived"];

/// One captured idea: the `ideas` row as the client sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaItem {
    pub id: i64,
    pub project: String,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub priority: String,
    /// Tags exactly as stored (usually a JSON array string, as the MCP writes).
    pub tags: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// One inter-agent blackboard note: the `blackboard_notes` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlackboardNote {
    pub id: i64,
    pub project: String,
    pub topic: String,
    pub sender: String,
    pub target: String,
    pub note: String,
    pub is_read: bool,
    pub created_at: String,
}

/// Arguments for creating an idea; omitted enums take the MCP defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIdeaInput {
    pub project: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub tags: Option<String>,
}

/// Partial update: only `Some` fields are written, `updated_at` always moves.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIdeaInput {
    pub id: i64,
    pub project: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub tags: Option<String>,
    pub status: Option<String>,
}

/// Arguments for posting a blackboard note; `target` defaults to `all`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostNoteInput {
    pub project: String,
    pub topic: String,
    pub sender: String,
    pub target: Option<String>,
    pub note: String,
}
