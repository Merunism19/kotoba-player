//! Subtitle parsing (ASS/SSA, SRT, WebVTT) into cues, sign/song classification, and JP↔EN alignment.
//!
//! Typical use: [`parse`] the bytes of a subtitle file, show the `Dialogue` cues in the UI,
//! hand [`Track::signs_only_ass`] to mpv so signs and songs render as authored, and use
//! [`align`] to pair Japanese and English dialogue.

mod ass;
mod classify;
mod decode;
mod plain;
mod time;

use serde::Serialize;

/// Subtitle file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Advanced SubStation Alpha / SubStation Alpha.
    Ass,
    /// SubRip.
    Srt,
    /// WebVTT.
    Vtt,
}

/// What a cue is for. Dialogue is drawn by the UI; signs and songs are drawn by mpv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CueKind {
    /// Spoken dialogue.
    Dialogue,
    /// On-screen text such as titles, signs and translator notes.
    Sign,
    /// Opening/ending/insert song lyrics (karaoke).
    Song,
}

/// One cleaned subtitle line. Serializes exactly as the TypeScript `Cue`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    /// Index in the track's cue list (cues are sorted by start).
    pub id: u32,
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// End time in milliseconds.
    pub end_ms: i64,
    /// Plain text; line breaks are `\n`.
    pub text: String,
    /// Dialogue, sign or song.
    pub kind: CueKind,
    /// ASS style name (not sent to the UI).
    #[serde(skip)]
    pub style: Option<String>,
}

/// A parsed subtitle file.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The format the file was parsed as.
    pub format: Format,
    /// All cues (dialogue, signs, songs), sorted by start; `id` is the index.
    pub cues: Vec<Cue>,
    header: String,
    events_format: String,
    sign_lines: Vec<String>,
    /// Cue id of each entry of `sign_lines` (same order).
    sign_ids: Vec<u32>,
    styles: Vec<String>,
}

/// Why a file could not be turned into a track.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The input had no content.
    #[error("subtitle file is empty")]
    Empty,
    /// The input was readable but held no usable cues.
    #[error("no subtitle cues found")]
    NoCues,
}

impl Track {
    /// Style names declared in an ASS file's styles section (empty for SRT/VTT).
    pub fn style_names(&self) -> &[String] {
        &self.styles
    }

    /// A valid ASS script holding only the sign and song events of this track
    /// (override tags intact) so mpv can render them exactly as authored.
    /// `None` if the track is not ASS or has no sign/song cues.
    pub fn signs_only_ass(&self) -> Option<String> {
        if self.format != Format::Ass || self.sign_lines.is_empty() {
            return None;
        }
        let mut out = String::new();
        if self.header.is_empty() {
            out.push_str("[Script Info]\nScriptType: v4.00+\n");
        } else {
            out.push_str(&self.header);
            out.push('\n');
        }
        out.push_str("\n[Events]\n");
        out.push_str(&self.events_format);
        out.push('\n');
        for l in &self.sign_lines {
            out.push_str(l);
            out.push('\n');
        }
        Some(out)
    }

    /// Like [`Track::signs_only_ass`], but every sign/song event takes its Start/End from the cue
    /// with the same id in `cues` (e.g. this track's cues after an auto-sync shifted them), so mpv
    /// draws the signs in step with the retimed dialogue. Override tags stay exactly as authored.
    pub fn signs_only_ass_timed(&self, cues: &[Cue]) -> Option<String> {
        let base = self.signs_only_ass()?;
        let fields: Vec<String> = self
            .events_format
            .split_once(':')
            .map(|(_, f)| {
                f.split(',')
                    .map(|s| s.trim().to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let (Some(si), Some(ei)) = (
            fields.iter().position(|f| f == "start"),
            fields.iter().position(|f| f == "end"),
        ) else {
            return Some(base);
        };
        let mut out = String::with_capacity(base.len());
        let head_end = base.len() - self.sign_lines.iter().map(|l| l.len() + 1).sum::<usize>();
        out.push_str(&base[..head_end]);
        for (line, id) in self.sign_lines.iter().zip(&self.sign_ids) {
            let cue = cues.iter().find(|c| c.id == *id);
            match (cue, line.split_once(':')) {
                (Some(c), Some((kind, rest))) => {
                    let mut parts: Vec<String> = rest
                        .splitn(fields.len().max(1), ',')
                        .map(str::to_string)
                        .collect();
                    if parts.len() > si.max(ei) {
                        parts[si] = time::format_ass(c.start_ms);
                        parts[ei] = time::format_ass(c.end_ms);
                        // Keep the space Aegisub writes after "Dialogue:".
                        let first = parts[0].trim_start().to_string();
                        parts[0] = first;
                        out.push_str(kind);
                        out.push_str(": ");
                        out.push_str(&parts.join(","));
                    } else {
                        out.push_str(line);
                    }
                }
                _ => out.push_str(line),
            }
            out.push('\n');
        }
        Some(out)
    }
}

fn detect(text: &str) -> Format {
    if text.trim_start().starts_with("WEBVTT") {
        return Format::Vtt;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("[script info]") || lower.contains("[events]") {
        Format::Ass
    } else {
        Format::Srt
    }
}

/// Parse subtitle bytes. Handles UTF-8 (with/without BOM), UTF-16 with BOM and a
/// Shift_JIS fallback. The format is taken from `hint`, or detected from the content.
/// Never panics; returns an error if the file is empty or has no cues.
pub fn parse(bytes: &[u8], hint: Option<Format>) -> Result<Track, Error> {
    if bytes.is_empty() {
        return Err(Error::Empty);
    }
    let text = decode::decode(bytes);
    let format = hint.unwrap_or_else(|| detect(&text));
    let mut track = match format {
        Format::Ass => {
            let data = ass::parse_ass(&text);
            // Cue ids are positions in `data.cues` (assigned below, no re-sort), so each sign line
            // keeps the id of its cue for retiming.
            let (sign_lines, sign_ids): (Vec<String>, Vec<u32>) = data
                .cues
                .iter()
                .zip(&data.raw)
                .enumerate()
                .filter(|(_, (c, _))| c.kind != CueKind::Dialogue)
                .map(|(i, (_, r))| (r.clone(), u32::try_from(i).unwrap_or(u32::MAX)))
                .unzip();
            Track {
                format,
                cues: data.cues,
                header: data.header,
                events_format: data.events_format,
                sign_lines,
                sign_ids,
                styles: data.styles,
            }
        }
        Format::Srt | Format::Vtt => {
            let mut cues = plain::parse_plain(&text);
            cues.sort_by_key(|c| c.start_ms);
            Track {
                format,
                cues,
                header: String::new(),
                events_format: String::new(),
                sign_lines: Vec::new(),
                sign_ids: Vec::new(),
                styles: Vec::new(),
            }
        }
    };
    if track.cues.is_empty() {
        return Err(Error::NoCues);
    }
    for (i, c) in track.cues.iter_mut().enumerate() {
        c.id = u32::try_from(i).unwrap_or(u32::MAX);
    }
    Ok(track)
}

/// Pair each Japanese dialogue cue with the English dialogue cues that overlap it by at
/// least 40% of the shorter of the two. Returns `(jp id, en ids sorted by start)` for every
/// Japanese dialogue cue (the list may be empty), ordered by Japanese id. An English cue
/// may appear under two Japanese cues.
pub fn align(jp: &[Cue], en: &[Cue]) -> Vec<(u32, Vec<u32>)> {
    let mut en_sorted: Vec<&Cue> = en.iter().filter(|c| c.kind == CueKind::Dialogue).collect();
    en_sorted.sort_by_key(|c| (c.start_ms, c.id));
    let mut out: Vec<(u32, Vec<u32>)> = jp
        .iter()
        .filter(|c| c.kind == CueKind::Dialogue)
        .map(|j| {
            let ids = en_sorted
                .iter()
                .take_while(|e| e.start_ms < j.end_ms)
                .filter(|e| {
                    let overlap = j
                        .end_ms
                        .min(e.end_ms)
                        .saturating_sub(j.start_ms.max(e.start_ms));
                    let shorter = (j.end_ms - j.start_ms).min(e.end_ms - e.start_ms);
                    overlap > 0 && i128::from(overlap) * 100 >= i128::from(shorter) * 40
                })
                .map(|e| e.id)
                .collect();
            (j.id, ids)
        })
        .collect();
    out.sort_by_key(|(id, _)| *id);
    out
}

/// Index of a dialogue cue active at `t_ms` (`start <= t < end`); the latest-starting one if
/// several are. `cues` is expected sorted by start, as returned by [`parse`].
pub fn cue_index_at(cues: &[Cue], t_ms: i64) -> Option<usize> {
    let upto = cues.partition_point(|c| c.start_ms <= t_ms);
    cues[..upto]
        .iter()
        .rposition(|c| c.kind == CueKind::Dialogue && t_ms < c.end_ms)
}
