//! Canonical error for foundational operations.
//!
//! The messages keep the `failed to …` / `refusing to …` wording the UI
//! surfaces, so moving this code into `orquestra-core` never changes what an
//! operator reads. `From<CoreError> for String` lets migrated call sites keep
//! their existing `Result<_, String>` signatures and use `?` unchanged.

use std::fmt;
use std::path::PathBuf;

/// Result alias for the crate's fallible APIs.
pub type CoreResult<T> = Result<T, CoreError>;

/// Failure from a foundational operation: storage layout, path resolution,
/// or a safety check on a runtime directory.
#[derive(Debug)]
pub enum CoreError {
    /// A required environment variable is unset.
    MissingEnv {
        /// Variable name, such as `HOME`.
        name: &'static str,
        /// What the variable was needed for, completing "cannot …".
        purpose: &'static str,
    },
    /// A filesystem operation on a single path failed.
    Io {
        /// Verb phrase completing "failed to …", such as `create`.
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    /// A filesystem operation that needs a trailing context phrase.
    IoWithContext {
        /// Verb phrase completing "failed to …", such as `create`.
        action: &'static str,
        path: PathBuf,
        /// Phrase appended after the path, such as `while seeding`.
        context: &'static str,
        source: std::io::Error,
    },
    /// A directory copy failed partway through.
    Copy {
        source_path: PathBuf,
        destination: PathBuf,
        source: std::io::Error,
    },
    /// A path failed a safety check, so the operation was refused.
    UnsafePath {
        /// Verb phrase completing "refusing to …", such as `stage in`.
        action: &'static str,
        path: PathBuf,
        reason: &'static str,
    },
}

impl CoreError {
    /// A required environment variable was not set.
    pub fn missing_env(name: &'static str, purpose: &'static str) -> Self {
        Self::MissingEnv { name, purpose }
    }

    /// A single-path filesystem operation failed.
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// A filesystem operation failed and the message needs a context phrase.
    pub fn io_with_context(
        action: &'static str,
        path: impl Into<PathBuf>,
        context: &'static str,
        source: std::io::Error,
    ) -> Self {
        Self::IoWithContext {
            action,
            path: path.into(),
            context,
            source,
        }
    }

    /// Copying one directory entry onto its destination failed.
    pub fn copy(
        source_path: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Copy {
            source_path: source_path.into(),
            destination: destination.into(),
            source,
        }
    }

    /// A path was refused because it was not what the operation required.
    pub fn unsafe_path(
        action: &'static str,
        path: impl Into<PathBuf>,
        reason: &'static str,
    ) -> Self {
        Self::UnsafePath {
            action,
            path: path.into(),
            reason,
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv { name, purpose } => {
                write!(formatter, "{name} is not set; cannot {purpose}")
            }
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "failed to {action} {}: {source}", path.display()),
            Self::IoWithContext {
                action,
                path,
                context,
                source,
            } => write!(
                formatter,
                "failed to {action} {} {context}: {source}",
                path.display()
            ),
            Self::Copy {
                source_path,
                destination,
                source,
            } => write!(
                formatter,
                "failed to copy {} -> {}: {source}",
                source_path.display(),
                destination.display()
            ),
            Self::UnsafePath {
                action,
                path,
                reason,
            } => write!(
                formatter,
                "refusing to {action} {}: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. }
            | Self::IoWithContext { source, .. }
            | Self::Copy { source, .. } => Some(source),
            Self::MissingEnv { .. } | Self::UnsafePath { .. } => None,
        }
    }
}

/// The desktop shell's IPC layer reports errors as plain strings, so the
/// migrated call sites keep working with `?` in `Result<_, String>` functions.
impl From<CoreError> for String {
    fn from(error: CoreError) -> Self {
        error.to_string()
    }
}
