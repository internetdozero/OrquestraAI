//! `orquestra-harness`: one MCP server definition, one dialect per harness.
//!
//! Every harness expects a different envelope around the same server, and
//! every write starts from the file already on disk so keys OrquestraAI does
//! not own (project state, hooks, plugin tables, hand-added servers) survive
//! untouched. JSON targets are edited in place; Codex goes through
//! `toml_edit`, which keeps its comments, key order and unrelated tables
//! byte-identical.
//!
//! The crate is pure: it holds no Tauri runtime, no catalog storage and no
//! file I/O. Callers read the config, hand the bytes over, and write the
//! result back atomically.

pub mod serializers;
pub mod server;

pub use serializers::{
    render_codex_toml, render_jsonc_block, render_mcp_config, render_mcp_servers,
};
pub use server::HarnessServer;
