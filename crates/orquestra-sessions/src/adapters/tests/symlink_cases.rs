//! Directory-symlink cases: a store folder replaced by a link must never be
//! listed, read through, or deleted into.
//!
//! Split by harness so each file stays small; the fixture harness is
//! re-exported for the children.

pub(super) use super::{adapter, TestHome};

mod claude;
mod freebuff;
mod others;
