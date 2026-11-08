//! SRT and WebVTT reader (one lenient parser for both).

use crate::time::parse_clock;
use crate::{Cue, CueKind};

const ENTITIES: [(&str, &str); 6] = [
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&nbsp;", " "),
    ("&lrm;", ""),
    ("&rlm;", ""),
];

/// Strip `<...>` markup, `{\...}` blocks and common HTML entities from a line.
/// Looks only a bounded distance for the closing bracket, so it stays linear.
fn clean_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find(['<', '{', '&']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let mut end = tail.len().min(120);
        while !tail.is_char_boundary(end) {
            end -= 1;
        }
        let window = &tail[..end];
        let skip = match tail.as_bytes()[0] {
            b'<' => window.find('>').filter(|&j| {
                let inner = &window[1..j];
                !inner.contains('<') && inner.starts_with(|c: char| c.is_alphabetic() || c == '/')
            }),
            b'{' if tail[1..].starts_with('\\') => window.find('}'),
            _ => None,
        };
        if let Some(j) = skip {
            rest = &tail[j + 1..];
            continue;
        }
        if let Some((e, r)) = ENTITIES.iter().find(|(e, _)| tail.starts_with(e)) {
            out.push_str(r);
            rest = &tail[e.len()..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn timing(line: &str) -> Option<(i64, i64)> {
    let (a, b) = line.split_once("-->")?;
    let start = parse_clock(a)?;
    let end = parse_clock(b.split_whitespace().next()?)?;
    (end >= start).then_some((start, end))
}

type Pending = Option<(i64, i64, Vec<String>)>;

fn finish(cur: Pending, out: &mut Vec<Cue>) {
    if let Some((start_ms, end_ms, lines)) = cur {
        let joined = lines
            .iter()
            .map(|l| clean_line(l).trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let text = joined.trim().to_string();
        if !text.is_empty() {
            out.push(Cue {
                id: 0,
                start_ms,
                end_ms,
                text,
                kind: CueKind::Dialogue,
                style: None,
            });
        }
    }
}

/// Parse SRT/VTT text (already decoded, `\n` newlines) into dialogue cues.
pub(crate) fn parse_plain(text: &str) -> Vec<Cue> {
    let mut out = Vec::new();
    let mut cur: Pending = None;
    for line in text.lines() {
        if let Some((s, e)) = timing(line) {
            let mut prev = cur.take();
            // An SRT index line directly before the timing line belongs to this cue.
            if let Some((_, _, lines)) = prev.as_mut()
                && lines.last().is_some_and(|l| {
                    !l.trim().is_empty() && l.trim().bytes().all(|b| b.is_ascii_digit())
                })
            {
                lines.pop();
            }
            finish(prev, &mut out);
            cur = Some((s, e, Vec::new()));
        } else if line.trim().is_empty() {
            finish(cur.take(), &mut out);
        } else if let Some((_, _, lines)) = cur.as_mut() {
            lines.push(line.to_string());
        }
    }
    finish(cur, &mut out);
    out
}
