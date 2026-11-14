//! Text shaping for the sessions table: titles that fit a cell, and the
//! encoded path segments some harnesses use for their session folders.

use crate::types::TITLE_LIMIT;

/// Collapse whitespace and trim a prompt down to something a table cell can
/// show without breaking layout. Counts characters, not bytes, so a title is
/// never cut inside a multi-byte glyph.
pub fn trim_title(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= TITLE_LIMIT {
        return collapsed;
    }

    let mut trimmed: String = collapsed.chars().take(TITLE_LIMIT - 1).collect();
    trimmed.push('…');
    trimmed
}

/// Best effort recovery of a working directory from an encoded path segment
/// (`%2Fhome%2Fsystem`, `/home/system`) used by Grok's session folders.
pub fn decode_path_hint(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('/') {
        return Some(raw.to_string());
    }
    if !raw.contains('%') {
        return None;
    }

    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}
