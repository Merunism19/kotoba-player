//! ASS text cleaning and the dialogue / sign / song heuristics.

use crate::CueKind;

/// What the override tags of one event told us.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Tags {
    /// `\k`, `\kf`, `\ko` or `\K` karaoke timing.
    pub karaoke: bool,
    /// `\pos` or `\move`.
    pub pos: bool,
    /// Any placement tag: `\pos`, `\move`, `\org` or `\an7`-`\an9`.
    pub placed: bool,
    /// Drawing mode `\p1` or higher.
    pub drawing: bool,
}

fn scan_tag(token: &str, tags: &mut Tags) {
    let t = token.trim();
    if t.starts_with("pos") || t.starts_with("move") {
        tags.pos = true;
        tags.placed = true;
    } else if t.starts_with("org") {
        tags.placed = true;
    } else if let Some(rest) = t.strip_prefix("an") {
        if matches!(rest.bytes().next(), Some(b'7'..=b'9')) {
            tags.placed = true;
        }
    } else if let Some(rest) = t.strip_prefix('p') {
        if !rest.is_empty()
            && rest.bytes().all(|b| b.is_ascii_digit())
            && rest.bytes().any(|b| b != b'0')
        {
            tags.drawing = true;
        }
    } else if let Some(rest) = t.strip_prefix(['k', 'K']) {
        let rest = rest.strip_prefix(['f', 'o']).unwrap_or(rest);
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            tags.karaoke = true;
        }
    }
}

/// Clean the Text field of an ASS event: drop `{...}` blocks (recording which
/// tags they held), turn `\N`/`\n` into a newline and `\h` into a space, trim.
/// An unclosed `{` is kept literally. Runs in linear time.
pub(crate) fn clean_ass_text(raw: &str) -> (String, Tags) {
    let mut tags = Tags::default();
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut no_more_close = false;
    while let Some(i) = rest.find(['{', '\\']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(inner) = tail.strip_prefix('{') {
            let close = if no_more_close { None } else { tail.find('}') };
            match close {
                Some(j) => {
                    for tok in tail[1..j].split('\\') {
                        scan_tag(tok, &mut tags);
                    }
                    rest = &tail[j + 1..];
                }
                None => {
                    no_more_close = true;
                    out.push('{');
                    rest = inner;
                }
            }
        } else {
            match tail[1..].chars().next() {
                Some('N' | 'n') => {
                    out.push('\n');
                    rest = &tail[2..];
                }
                Some('h') => {
                    out.push(' ');
                    rest = &tail[2..];
                }
                _ => {
                    out.push('\\');
                    rest = &tail[1..];
                }
            }
        }
    }
    out.push_str(rest);
    (out.trim().to_string(), tags)
}

/// Split a style name into lowercase words (on non-alphanumerics and camelCase).
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

const PLAIN: [&str; 14] = [
    "default",
    "main",
    "dialogue",
    "italics",
    "italic",
    "top",
    "flashback",
    "alt",
    "overlap",
    "narrator",
    "thought",
    "thoughts",
    "internal",
    "bottom",
];

fn strip_digits(w: &str) -> &str {
    w.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Classify one event from its style name and tags (first pass; the
/// "positioned line overlapping ordinary dialogue" rule needs the whole track
/// and is applied afterwards by the ASS reader).
///
/// Heuristics, in order (style matching is on whole words / well-known
/// substrings, case-insensitive, rather than a raw regex, so "Speed" is not "ed"):
/// 1. Karaoke tags (`\k`, `\kf`, `\ko`, `\K`) -> `Song`.
/// 2. A "plain dialogue" style (Default, Main, Dialogue, Italics, Top,
///    Flashback, Alt, Overlap, Narrator, ...) never counts as a sign/song style.
/// 3. Style words `op`/`ed` (digits ignored) or containing song, karaoke, kfx,
///    lyric, insert, opening, ending -> `Song`.
/// 4. Style containing sign, title (but not subtitle), note, screen, board,
///    text, caption, or the word `tl` -> `Sign`.
/// 5. A placement tag (`\pos`, `\move`, `\org`, `\an7`-`\an9`) on a style that
///    is not a plain dialogue style -> `Sign`.
/// 6. Otherwise `Dialogue` (later promoted to `Sign` by the ASS reader when a
///    `\pos`-positioned line overlaps ordinary dialogue in time).
pub(crate) fn classify_event(style: &str, tags: &Tags) -> CueKind {
    if tags.karaoke {
        return CueKind::Song;
    }
    let ws = words(style);
    if !ws.is_empty() && ws.iter().all(|w| PLAIN.contains(&strip_digits(w))) {
        return CueKind::Dialogue;
    }
    let lower = style.to_lowercase().replace("subtitle", "");
    let song_word = ws.iter().any(|w| matches!(strip_digits(w), "op" | "ed"));
    let song_part = [
        "song", "karaoke", "kfx", "lyric", "insert", "opening", "ending",
    ];
    if song_word || song_part.iter().any(|k| lower.contains(k)) {
        return CueKind::Song;
    }
    let sign_part = [
        "sign", "title", "note", "screen", "board", "text", "caption",
    ];
    if sign_part.iter().any(|k| lower.contains(k)) || ws.iter().any(|w| w == "tl") || tags.placed {
        return CueKind::Sign;
    }
    CueKind::Dialogue
}
