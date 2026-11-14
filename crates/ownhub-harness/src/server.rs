//! The dialect-neutral shape every serializer renders.

use std::collections::HashMap;

/// One MCP server as the master catalog defines it, stripped of everything
/// app-specific (`global_enabled`, `enabled_harnesses`, official tier).
///
/// The serializers only ever read these fields, so a caller can project its
/// own catalog type into this one without dragging it along.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessServer {
    /// Stable id: the key every harness config nests the entry under.
    pub id: String,
    /// Executable for a stdio server.
    pub command: String,
    /// Arguments for a stdio server.
    pub args: Vec<String>,
    /// Environment variables merged into the entry, never replacing foreign
    /// ones on the same server.
    pub env: HashMap<String, String>,
    /// When set, the server is remote: `command`/`args` are dropped.
    pub url: Option<String>,
    /// HTTP headers for a remote server.
    pub headers: HashMap<String, String>,
}

impl HarnessServer {
    /// A minimal stdio server, for callers with no env, URL or headers.
    pub fn stdio(id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            command: command.into(),
            ..Self::default()
        }
    }

    /// True when the server is reached over HTTP rather than spawned.
    pub fn is_remote(&self) -> bool {
        self.url.is_some()
    }
}
