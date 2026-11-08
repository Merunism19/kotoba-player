//! Per-file memory: resume position, subtitle offset, chosen subtitle tracks, and the "Continue watching" list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// What we remember about one video file (keyed by its full path).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileState {
    pub title: String,
    pub offset_ms: i64,
    pub jp_sub_id: Option<u32>,
    pub en_sub_id: Option<u32>,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Unix milliseconds of the last open.
    pub last_opened_ms: u64,
    /// Position the current thumbnail was taken at.
    pub thumb_pos_ms: Option<u64>,
    /// Japanese cue ids already sent to Anki (gold marks on the seek bar).
    pub mined_cue_ids: Vec<u32>,
    /// Subtitle files the user added for this video (dragged in), read in place.
    pub extra_subs: Vec<String>,
    /// Auto-sync result per subtitle track id: one shift (ms) per cue, in cue order.
    pub retimed: BTreeMap<u32, Vec<i64>>,
}

/// The whole library store (see `stores::Store::Library`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub version: u32,
    pub files: BTreeMap<String, FileState>,
}

/// How many files the store keeps (oldest dropped first).
const MAX_FILES: usize = 500;

impl FileState {
    /// Watched to the end (credits count as the end): within 90 s of the end, or past 95 %.
    pub fn finished(&self) -> bool {
        self.duration_ms > 0
            && (self.position_ms + 90_000 >= self.duration_ms
                || self.position_ms * 100 >= self.duration_ms * 95)
    }

    /// Where to resume, if anywhere (not near the start, not finished).
    pub fn resume_ms(&self) -> Option<u64> {
        (self.position_ms >= 10_000 && !self.finished()).then_some(self.position_ms)
    }
}

impl Library {
    /// The entry for `path`, created if missing.
    pub fn entry(&mut self, path: &str) -> &mut FileState {
        self.files.entry(path.to_string()).or_default()
    }

    /// Mark a file as opened now (and prune the store).
    pub fn touch(&mut self, path: &str, title: &str) {
        let e = self.entry(path);
        e.title = title.to_string();
        e.last_opened_ms = now_ms();
        self.prune();
    }

    /// Unfinished files, most recently opened first, at most `n`.
    pub fn continue_watching(&self, n: usize) -> Vec<(&String, &FileState)> {
        let mut v: Vec<_> = self
            .files
            .iter()
            .filter(|(_, s)| s.position_ms > 0 && !s.finished())
            .collect();
        v.sort_by_key(|(_, s)| std::cmp::Reverse(s.last_opened_ms));
        v.truncate(n);
        v
    }

    fn prune(&mut self) {
        if self.files.len() <= MAX_FILES {
            return;
        }
        let mut by_age: Vec<(String, u64)> = self
            .files
            .iter()
            .map(|(k, v)| (k.clone(), v.last_opened_ms))
            .collect();
        by_age.sort_by_key(|(_, t)| *t);
        for (k, _) in by_age.into_iter().take(self.files.len() - MAX_FILES) {
            self.files.remove(&k);
        }
    }
}

/// Current Unix time in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Unix milliseconds as an ISO-8601 UTC timestamp, e.g. `2026-11-08T20:48:57Z`.
pub fn iso8601(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Display title from a file name: drops the extension, release-group brackets and dots/underscores.
pub fn title_from_path(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    let mut out = String::new();
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = (depth - 1).max(0),
            _ if depth > 0 => {}
            '_' | '.' => out.push(' '),
            _ => out.push(c),
        }
    }
    let t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { name.to_string() } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_rules() {
        let s = |p, d| FileState {
            position_ms: p,
            duration_ms: d,
            ..Default::default()
        };
        assert_eq!(
            s(5_000, 1_440_000).resume_ms(),
            None,
            "too close to the start"
        );
        assert_eq!(s(600_000, 1_440_000).resume_ms(), Some(600_000));
        assert_eq!(s(1_380_000, 1_440_000).resume_ms(), None, "in the credits");
        assert!(s(1_400_000, 1_440_000).finished());
        assert!(!s(0, 0).finished());
    }

    #[test]
    fn continue_watching_order_and_filter() {
        let mut lib = Library::default();
        for (p, pos, t) in [
            ("a", 600_000, 1),
            ("b", 1_430_000, 3),
            ("c", 300_000, 2),
            ("d", 0, 4),
        ] {
            let e = lib.entry(p);
            e.position_ms = pos;
            e.duration_ms = 1_440_000;
            e.last_opened_ms = t;
        }
        let keys: Vec<_> = lib
            .continue_watching(10)
            .into_iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["c", "a"]);
    }

    #[test]
    fn prune_keeps_newest() {
        let mut lib = Library::default();
        for i in 0..(MAX_FILES + 5) {
            lib.entry(&format!("f{i}")).last_opened_ms = i as u64;
        }
        lib.touch("new", "New");
        assert_eq!(lib.files.len(), MAX_FILES);
        assert!(lib.files.contains_key("new") && !lib.files.contains_key("f0"));
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_794_169_737_000), "2026-11-08T20:28:57Z");
        assert_eq!(iso8601(951_782_400_000), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn titles() {
        assert_eq!(
            title_from_path(r"D:\anime\[SubsPlease] Sousou no Frieren - 07 (1080p) [ABCD1234].mkv"),
            "Sousou no Frieren - 07"
        );
        assert_eq!(
            title_from_path("/x/Some_Show.S01E07.mkv"),
            "Some Show S01E07"
        );
        assert_eq!(title_from_path("[only brackets].mkv"), "[only brackets]");
    }

    #[test]
    fn state_json_is_tolerant() {
        let s: FileState =
            serde_json::from_str(r#"{"offsetMs": -300, "unknownField": 1}"#).unwrap();
        assert_eq!(s.offset_ms, -300);
        assert_eq!(s.position_ms, 0);
    }
}
