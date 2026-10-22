//! Adapter tests for the stores that keep their own on-disk layout.
//!
//! Split by store family so each file stays well under the line cap; the
//! fixture harness is re-exported for the children.

pub(super) use super::{adapter, TestHome};

mod file_symlink;
mod freebuff;
mod indexes;
