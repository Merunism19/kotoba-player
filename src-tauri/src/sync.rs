//! Automatic subtitle sync: retime a subtitle track against a reference track with alass (handles ad breaks / splits).

use alass_core::{NoProgressHandler, TimeDelta, TimePoint, TimeSpan};
use subtitles::{Cue, CueKind};

/// Summary of a sync for the UI.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    /// Median shift applied (ms; positive = later).
    pub median_shift_ms: i64,
    /// Smallest and largest shift (differ when alass split the track, e.g. around an ad break).
    pub min_shift_ms: i64,
    pub max_shift_ms: i64,
    /// Lines whose timing changed.
    pub changed: usize,
}

fn span(c: &Cue) -> TimeSpan {
    let end = if c.end_ms > c.start_ms {
        c.end_ms
    } else {
        c.start_ms + 1
    };
    TimeSpan::new(TimePoint::from(c.start_ms), TimePoint::from(end))
}

/// Per-cue shifts (ms) that line `list` up with `reference` (its dialogue lines are the timing
/// truth). Every cue of `list` (signs too) gets a shift, in order.
pub fn deltas(reference: &[Cue], list: &[Cue]) -> Vec<i64> {
    let refs: Vec<TimeSpan> = reference
        .iter()
        .filter(|c| c.kind == CueKind::Dialogue)
        .map(span)
        .collect();
    let spans: Vec<TimeSpan> = list.iter().map(span).collect();
    if refs.is_empty() || spans.is_empty() {
        return vec![0; list.len()];
    }
    // split penalty 7 is alass's recommended default; speed optimization 3 keeps it well under a second.
    let (d, _score) = alass_core::align(
        &refs,
        &spans,
        7.0,
        Some(3.0),
        alass_core::standard_scoring,
        NoProgressHandler,
    );
    d.into_iter().map(|x: TimeDelta| x.as_i64()).collect()
}

/// Apply shifts to cues (in order); cues beyond `shifts` stay as they are.
pub fn apply(cues: &mut [Cue], shifts: &[i64]) {
    for (c, s) in cues.iter_mut().zip(shifts) {
        c.start_ms = (c.start_ms + s).max(0);
        c.end_ms = (c.end_ms + s).max(c.start_ms);
    }
}

/// Describe a set of shifts.
pub fn summarize(shifts: &[i64]) -> SyncSummary {
    let mut sorted = shifts.to_vec();
    sorted.sort_unstable();
    SyncSummary {
        median_shift_ms: sorted.get(sorted.len() / 2).copied().unwrap_or(0),
        min_shift_ms: sorted.first().copied().unwrap_or(0),
        max_shift_ms: sorted.last().copied().unwrap_or(0),
        changed: shifts.iter().filter(|s| **s != 0).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(id: u32, start: i64, end: i64) -> Cue {
        Cue {
            id,
            start_ms: start,
            end_ms: end,
            text: format!("line {id}"),
            kind: CueKind::Dialogue,
            style: None,
        }
    }

    /// 40 irregular lines; the second half after a 90 s "ad break".
    fn reference() -> Vec<Cue> {
        let mut t = 1_000;
        (0..40)
            .map(|i| {
                let len = 900 + (i * 377 % 2100) as i64;
                let gap = 400 + (i * 613 % 1800) as i64 + if i == 20 { 90_000 } else { 0 };
                let c = cue(i, t, t + len);
                t += len + gap;
                c
            })
            .collect()
    }

    #[test]
    fn finds_a_constant_shift() {
        let r = reference();
        let late: Vec<Cue> = r
            .iter()
            .map(|c| cue(c.id, c.start_ms + 2_350, c.end_ms + 2_350))
            .collect();
        let d = deltas(&r, &late);
        let s = summarize(&d);
        assert!((s.median_shift_ms + 2_350).abs() <= 20, "{s:?}");
        let mut fixed = late.clone();
        apply(&mut fixed, &d);
        assert!((fixed[10].start_ms - r[10].start_ms).abs() <= 20);
    }

    #[test]
    fn handles_a_split_like_an_ad_break() {
        let r = reference();
        // Release without the ad: second half is 90 s earlier, and everything is 1 s late.
        let other: Vec<Cue> = r
            .iter()
            .map(|c| {
                let shift = 1_000 - if c.id >= 21 { 90_000 } else { 0 };
                cue(c.id, c.start_ms + shift, c.end_ms + shift)
            })
            .collect();
        let d = deltas(&r, &other);
        let s = summarize(&d);
        // First half 1 s early-corrected, second half moved back over the 90 s break.
        assert!((s.min_shift_ms + 1_000).abs() <= 20 && (s.max_shift_ms - 89_000).abs() <= 20, "{s:?}");
        let mut fixed = other.clone();
        apply(&mut fixed, &d);
        for (a, b) in fixed.iter().zip(&r) {
            assert!(
                (a.start_ms - b.start_ms).abs() <= 50,
                "cue {} off by {}",
                a.id,
                a.start_ms - b.start_ms
            );
        }
    }

    #[test]
    fn empty_inputs_are_no_ops() {
        assert_eq!(deltas(&[], &[cue(0, 0, 10)]), vec![0]);
        assert!(deltas(&reference(), &[]).is_empty());
        assert_eq!(summarize(&[]).changed, 0);
    }
}
