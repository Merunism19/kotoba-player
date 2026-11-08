//! Clock parsing shared by the ASS, SRT and VTT readers.

/// Parse digits only (1..=9 of them) into a number.
fn digits(s: &str) -> Option<i64> {
    if s.is_empty() || s.len() > 9 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Milliseconds as an ASS timestamp `H:MM:SS.cc` (negative clamps to 0; centiseconds round down).
pub(crate) fn format_ass(ms: i64) -> String {
    let ms = ms.max(0);
    let cs = (ms / 10) % 100;
    let s = ms / 1000;
    format!("{}:{:02}:{:02}.{:02}", s / 3600, s / 60 % 60, s % 60, cs)
}

/// Parse `H:MM:SS.cc`, `HH:MM:SS,mmm` or `MM:SS.mmm` into milliseconds.
/// The fraction is a decimal fraction of a second (2 digits = centiseconds).
pub(crate) fn parse_clock(s: &str) -> Option<i64> {
    let mut parts = s.trim().split(':');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let (h, m, sec) = match third {
        Some(t) => (digits(first)?, digits(second)?, t),
        None => (0, digits(first)?, second),
    };
    let (whole, frac) = sec.split_once(['.', ',']).unwrap_or((sec, ""));
    let secs = digits(whole)?;
    let mut ms = 0i64;
    let mut scale = 100i64;
    for b in frac.bytes().take(3) {
        if !b.is_ascii_digit() {
            return None;
        }
        ms += i64::from(b - b'0') * scale;
        scale /= 10;
    }
    Some(((h * 60 + m) * 60 + secs) * 1000 + ms)
}
