//! ASS/SSA reader.

use crate::classify::{Tags, classify_event, clean_ass_text};
use crate::time::parse_clock;
use crate::{Cue, CueKind};

const DEFAULT_EVENT_FORMAT: &str =
    "Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text";

/// Everything the ASS reader extracts.
pub(crate) struct AssData {
    /// Cues sorted by start, ids not yet assigned.
    pub cues: Vec<Cue>,
    /// Raw `Dialogue:` line of each cue (same order as `cues`).
    pub raw: Vec<String>,
    /// `[Script Info]` and styles sections, verbatim.
    pub header: String,
    /// The `Format:` line of `[Events]`.
    pub events_format: String,
    /// Style names from the styles section.
    pub styles: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Other,
    Info,
    Styles,
    Events,
}

fn field_index(fields: &[String], name: &str, default: usize) -> usize {
    fields.iter().position(|f| f == name).unwrap_or(default)
}

fn format_fields(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|f| f.trim().to_ascii_lowercase())
        .collect()
}

struct Event {
    cue: Cue,
    raw: String,
    tags: Tags,
}

/// Parse a decoded ASS script.
pub(crate) fn parse_ass(text: &str) -> AssData {
    let mut section = Section::Other;
    let mut info = String::new();
    let mut styles_text = String::new();
    let mut style_fields: Vec<String> = Vec::new();
    let mut styles = Vec::new();
    let mut events_format = String::new();
    let mut ev_fields = format_fields(DEFAULT_EVENT_FORMAT);
    let mut events: Vec<Event> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let name = trimmed.to_ascii_lowercase();
            section = if name == "[script info]" {
                Section::Info
            } else if name.starts_with("[v4") && name.contains("styles") {
                Section::Styles
            } else if name == "[events]" {
                Section::Events
            } else {
                Section::Other
            };
        }
        match section {
            Section::Info => {
                info.push_str(line.trim_end());
                info.push('\n');
            }
            Section::Styles => {
                styles_text.push_str(line.trim_end());
                styles_text.push('\n');
                if let Some((key, value)) = trimmed.split_once(':') {
                    match key.trim().to_ascii_lowercase().as_str() {
                        "format" => style_fields = format_fields(value),
                        "style" => {
                            let idx = field_index(&style_fields, "name", 0);
                            if let Some(n) = value.trim_start().split(',').nth(idx) {
                                styles.push(n.trim().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Section::Events => {
                let Some((key, value)) = trimmed.split_once(':') else {
                    continue;
                };
                match key.trim().to_ascii_lowercase().as_str() {
                    "format" => {
                        events_format = format!("Format:{value}");
                        ev_fields = format_fields(value);
                    }
                    "dialogue" => {
                        if let Some(ev) = parse_event(value, &ev_fields, trimmed) {
                            events.push(ev);
                        }
                    }
                    _ => {}
                }
            }
            Section::Other => {}
        }
    }

    promote_overlapping_positioned(&mut events);
    events.sort_by_key(|e| e.cue.start_ms);
    if events_format.is_empty() {
        events_format = format!("Format: {DEFAULT_EVENT_FORMAT}");
    }
    let header = [
        info.trim_end_matches('\n'),
        styles_text.trim_end_matches('\n'),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
    let (cues, raw) = events.into_iter().map(|e| (e.cue, e.raw)).unzip();
    AssData {
        cues,
        raw,
        header,
        events_format,
        styles,
    }
}

fn parse_event(value: &str, fields: &[String], raw: &str) -> Option<Event> {
    let n = fields.len().max(1);
    let parts: Vec<&str> = value.trim_start().splitn(n, ',').collect();
    if parts.len() < n {
        return None;
    }
    let start_ms = parse_clock(parts.get(field_index(fields, "start", 1))?)?;
    let end_ms = parse_clock(parts.get(field_index(fields, "end", 2))?)?;
    if end_ms < start_ms {
        return None;
    }
    let style = parts
        .get(field_index(fields, "style", 3))?
        .trim()
        .to_string();
    let (text, tags) = clean_ass_text(parts[n - 1]);
    if text.is_empty() || tags.drawing {
        return None;
    }
    let kind = classify_event(&style, &tags);
    Some(Event {
        cue: Cue {
            id: 0,
            start_ms,
            end_ms,
            text,
            kind,
            style: Some(style),
        },
        raw: raw.to_string(),
        tags,
    })
}

/// Promote `\pos`-positioned dialogue that overlaps un-positioned dialogue in
/// time to `Sign` (classification rule 6 in `classify_event`).
fn promote_overlapping_positioned(events: &mut [Event]) {
    let is_plain_dialogue = |e: &Event| e.cue.kind == CueKind::Dialogue && !e.tags.pos;
    let mut spans: Vec<(i64, i64)> = events
        .iter()
        .filter(|e| is_plain_dialogue(e))
        .map(|e| (e.cue.start_ms, e.cue.end_ms))
        .collect();
    spans.sort_unstable();
    let mut max_end = Vec::with_capacity(spans.len());
    let mut m = i64::MIN;
    for s in &spans {
        m = m.max(s.1);
        max_end.push(m);
    }
    for e in events.iter_mut() {
        if e.cue.kind == CueKind::Dialogue && e.tags.pos {
            let upto = spans.partition_point(|s| s.0 < e.cue.end_ms);
            if upto > 0 && max_end[upto - 1] > e.cue.start_ms {
                e.cue.kind = CueKind::Sign;
            }
        }
    }
}
