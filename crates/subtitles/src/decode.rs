//! Byte decoding: UTF-8 (with or without BOM), UTF-16 LE/BE with BOM, Shift_JIS fallback.

use std::borrow::Cow;

use encoding_rs::{SHIFT_JIS, UTF_16BE, UTF_16LE};

/// Decode subtitle bytes to text. Never fails; invalid sequences become U+FFFD.
/// Line endings are normalised to `\n`.
pub(crate) fn decode(bytes: &[u8]) -> String {
    let text: Cow<'_, str> = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        UTF_16LE.decode_without_bom_handling(rest).0
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        UTF_16BE.decode_without_bom_handling(rest).0
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        Cow::Borrowed(s)
    } else {
        SHIFT_JIS.decode_without_bom_handling(bytes).0
    };
    normalize_newlines(text.trim_start_matches('\u{feff}'))
}

fn normalize_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}
