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
    /// English lines moved to follow the (synced) Japanese track, so each sits under its line.
    pub follow_changed: usize,
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
    align_to(&refs, list)
}

/// Sample rate of the PCM that `speech_spans` reads (WebRTC's detector works at 8 kHz).
pub const VAD_RATE: u32 = 8_000;

/// Speech found in the audio: rough spans for alass, and precise voice onsets for the fine step.
#[derive(Debug, Clone, Default)]
pub struct Speech {
    /// (start, end) ms, in 10 ms frames, gaps < 200 ms bridged.
    pub spans: Vec<(i64, i64)>,
    /// When a voice starts after a pause, to the millisecond (see `refine_onset`).
    pub onsets: Vec<i64>,
}

/// Where people speak in 8 kHz mono 16-bit PCM.
///
/// Anime has music under most dialogue, and WebRTC's voice detector alone calls most music "voice"
/// (65-100% of music-only frames in our test, even in its strictest mode). So a frame counts as
/// speech only if (1) the detector says voice AND (2) it is clearly louder than the local background:
/// its energy (pre-emphasised, so bass and kick drums count less) is at least `CONTRAST_DB` above the
/// 20th percentile of the surrounding ±3 s, which is the music bed between lines. Then gaps < 200 ms
/// are bridged (pauses inside a line) and blips < 200 ms dropped (drum hits, breaths).
pub fn detect_speech(pcm: &[i16]) -> Speech {
    use webrtc_vad::{SampleRate, Vad, VadMode};
    const FRAME: usize = (VAD_RATE / 100) as usize; // 10 ms
    const WINDOW: usize = 300; // ±3 s of frames for the background level
    const CONTRAST_DB: f32 = 6.0;
    let n = pcm.len() / FRAME;
    if n == 0 {
        return Speech::default();
    }
    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate8kHz, VadMode::VeryAggressive);
    let mut voiced = Vec::with_capacity(n);
    let mut energy = Vec::with_capacity(n);
    let mut prev = 0f32;
    for frame in pcm.as_chunks::<FRAME>().0 {
        voiced.push(vad.is_voice_segment(frame).unwrap_or(false));
        let mut sum = 0f32;
        for &x in frame {
            let x = x as f32;
            let y = x - 0.95 * prev;
            prev = x;
            sum += y * y;
        }
        energy.push(10.0 * (sum / FRAME as f32 + 1.0).log10());
    }
    // Background level: 20th percentile of the window around each frame, computed every 10 frames
    // (100 ms) to stay cheap on long files.
    let mut floor = vec![0f32; n];
    let mut buf: Vec<f32> = Vec::with_capacity(2 * WINDOW + 1);
    for c in (0..n).step_by(10) {
        let (a, b) = (c.saturating_sub(WINDOW), (c + WINDOW + 1).min(n));
        buf.clear();
        buf.extend_from_slice(&energy[a..b]);
        let k = buf.len() / 5;
        let (_, v, _) = buf.select_nth_unstable_by(k, |x, y| x.total_cmp(y));
        let v = *v;
        for f in &mut floor[c..(c + 10).min(n)] {
            *f = v;
        }
    }
    let speech = |i: usize| voiced[i] && energy[i] - floor[i] >= CONTRAST_DB;
    let mut out: Vec<(i64, i64)> = Vec::new();
    let mut run: Option<i64> = None;
    for i in 0..n {
        let t = i as i64 * 10;
        match (speech(i), run) {
            (true, None) => run = Some(t),
            (false, Some(s)) => {
                out.push((s, t));
                run = None;
            }
            _ => {}
        }
    }
    if let Some(s) = run {
        out.push((s, n as i64 * 10));
    }
    // Onsets: a voice starting after >= 150 ms of no speech, refined to the millisecond.
    let mut onsets = Vec::new();
    let mut last_end = i64::MIN / 2;
    for (s, e) in &out {
        if s - last_end >= 150 && e - s >= 60 {
            onsets.push(refine_onset(pcm, *s, floor[(*s / 10) as usize]));
        }
        last_end = *e;
    }
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(out.len());
    for (s, e) in out {
        match merged.last_mut() {
            Some(last) if s - last.1 < 200 => last.1 = e,
            _ => merged.push((s, e)),
        }
    }
    merged.retain(|(s, e)| e - s >= 200);
    Speech {
        spans: merged,
        onsets,
    }
}

/// The voice onset near `rough` (ms, from 10 ms frames) to 1 ms: the first moment from 200 ms
/// before it where the energy of a 10 ms window centred there rises `ONSET_DB` above the background
/// (`floor_db`, same pre-emphasised energy measure) and stays there for 20 ms.
fn refine_onset(pcm: &[i16], rough: i64, floor_db: f32) -> i64 {
    const ONSET_DB: f32 = 2.0;
    const PER_MS: i64 = (VAD_RATE / 1000) as i64; // samples per ms
    let energy_at = |t: i64| -> f32 {
        let (a, b) = ((t - 5) * PER_MS, (t + 5) * PER_MS);
        if a < 1 || b as usize > pcm.len() {
            return f32::MIN;
        }
        let mut sum = 0f32;
        for i in a as usize..b as usize {
            let y = pcm[i] as f32 - 0.95 * pcm[i - 1] as f32;
            sum += y * y;
        }
        10.0 * (sum / (b - a) as f32 + 1.0).log10()
    };
    let level: Vec<f32> = (rough - 200..rough + 60).map(energy_at).collect();
    let above = |i: usize| level[i] >= floor_db + ONSET_DB;
    (0..level.len().saturating_sub(20))
        .find(|&i| (i..i + 20).all(above))
        .map_or(rough, |i| rough - 200 + i as i64)
}

/// Audio sync leaves a track alone when no line would move more than this (ms): within what the
/// onset measurement can tell apart.
pub const IN_SYNC_MS: i64 = 20;

/// Where a synced line starts relative to its voice: this much before it (ms), the usual subtitle
/// lead-in so the text is up as the line is spoken. Audio can't know a subber's own lead-in, so
/// every synced line uses this one.
pub const LEAD_MS: i64 = 100;

/// Per-cue shifts (ms) that line the dialogue of `list` up with detected speech (`speech_spans`).
/// Signs and songs move with the dialogue but don't steer it (they don't match voices).
pub fn deltas_to_speech(speech: &Speech, list: &[Cue]) -> Vec<i64> {
    let refs: Vec<TimeSpan> = speech
        .spans
        .iter()
        .map(|(s, e)| TimeSpan::new(TimePoint::from(*s), TimePoint::from((*e).max(s + 1))))
        .collect();
    let dialogue: Vec<usize> = (0..list.len())
        .filter(|i| list[*i].kind == CueKind::Dialogue)
        .collect();
    if dialogue.is_empty() {
        return vec![0; list.len()];
    }
    let only: Vec<Cue> = dialogue.iter().map(|i| list[*i].clone()).collect();
    let mut d = align_to(&refs, &only);
    fine_tune(&speech.onsets, &only, &mut d);
    // If no line would move more than the measurement can tell, the track was already in sync:
    // leave it exactly as authored.
    if d.iter().all(|x| x.abs() <= IN_SYNC_MS) {
        return vec![0; list.len()];
    }
    // Every cue takes the shift of the nearest dialogue line before it (or the first one).
    let mut out = Vec::with_capacity(list.len());
    let mut k = 0;
    for i in 0..list.len() {
        while k + 1 < dialogue.len() && dialogue[k + 1] <= i {
            k += 1;
        }
        out.push(d[k]);
    }
    out
}

/// The fine step after alass. alass gets each part of the track (between splits) right to within
/// the slack subtitles have around the voice (~0.1-0.2 s, the same for every line of the part). For
/// each part, measure how far each line's start + `LEAD_MS` is from the nearest voice onset (within
/// 400 ms) and move the part by the median of that, so lines start `LEAD_MS` before their voices.
fn fine_tune(onsets: &[i64], lines: &[Cue], d: &mut [i64]) {
    let mut a = 0;
    while a < d.len() {
        let mut b = a + 1;
        while b < d.len() && d[b] == d[a] {
            b += 1;
        }
        let mut miss: Vec<i64> = (a..b)
            .filter_map(|i| {
                let want = lines[i].start_ms + d[i] + LEAD_MS;
                let k = onsets.partition_point(|o| *o < want);
                [k.checked_sub(1), Some(k)]
                    .into_iter()
                    .flatten()
                    .filter_map(|k| onsets.get(k))
                    .map(|o| o - want)
                    .min_by_key(|m| m.abs())
                    .filter(|m| m.abs() <= 400)
            })
            .collect();
        // Enough lines must agree (a part of 1-2 lines, e.g. a stray sign, stays as alass put it).
        if miss.len() >= 3 {
            miss.sort_unstable();
            let corr = miss[miss.len() / 2];
            for x in &mut d[a..b] {
                *x += corr;
            }
        }
        a = b;
    }
}

/// Shifts that make the English `list` follow the (already synced) Japanese `jp` track, so each
/// English line overlaps the Japanese line it translates. English that already lines up (no line
/// would move more than `IN_SYNC_MS`) is left exactly as authored.
pub fn deltas_follow(jp: &[Cue], list: &[Cue]) -> Vec<i64> {
    let d = deltas(jp, list);
    if d.iter().all(|x| x.abs() <= IN_SYNC_MS) {
        return vec![0; list.len()];
    }
    d
}

/// alass: shifts for `list` against reference spans.
fn align_to(refs: &[TimeSpan], list: &[Cue]) -> Vec<i64> {
    let spans: Vec<TimeSpan> = list.iter().map(span).collect();
    if refs.is_empty() || spans.is_empty() {
        return vec![0; list.len()];
    }
    // Split penalty 7 is alass's recommended default. Its speed optimization cost ~120 ms of accuracy on
    // a short track, so it's only used for very long ones (an episode has ~300-400 lines).
    let speed = (spans.len() > 1500).then_some(3.0);
    let (d, _score) = alass_core::align(
        refs,
        &spans,
        7.0,
        speed,
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
        follow_changed: 0,
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
        assert!(
            (s.min_shift_ms + 1_000).abs() <= 20 && (s.max_shift_ms - 89_000).abs() <= 20,
            "{s:?}"
        );
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

    /// Real speech (Windows voices) under music, with subs 2.7 s late (`const`) or split around an
    /// ad break (`split`): after audio sync, lines must start where the speech does.
    /// Needs `node scripts/make-speech-sample.mjs` and the bundled ffmpeg; skipped otherwise.
    #[test]
    fn syncs_to_real_speech() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join("../samples/speech");
        let tools = crate::media::Tools::find(&[root.join("lib")]);
        let (Some(tools), true) = (tools, dir.join("truth.json").is_file()) else {
            eprintln!("skipping: samples/speech or bundled tools missing");
            return;
        };
        let truth: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("truth.json")).unwrap()).unwrap();
        let starts: Vec<i64> = truth["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["start"].as_i64().unwrap())
            .collect();
        let scratch = std::env::temp_dir().join(format!("kp-vad-{}.pcm", std::process::id()));
        let t0 = std::time::Instant::now();
        let pcm = crate::media::audio_pcm(&tools, &dir.join("const/Ep.mkv"), 0, VAD_RATE, &scratch)
            .unwrap();
        let t_decode = t0.elapsed();
        let speech = detect_speech(&pcm);
        let t_vad = t0.elapsed() - t_decode;
        for case in ["exact", "const", "split", "messy"] {
            let bytes = std::fs::read(dir.join(case).join("Ep.ja.srt")).unwrap();
            let mut cues = subtitles::parse(&bytes, None).unwrap().cues;
            // The true start of each cue (None: lyrics/signs that nobody speaks).
            let expect: Vec<Option<i64>> = match std::fs::read(dir.join(case).join("expect.json")) {
                Ok(b) => serde_json::from_slice(&b).unwrap(),
                Err(_) => starts.iter().map(|t| Some(*t)).collect(),
            };
            assert_eq!(expect.len(), cues.len());
            let t1 = std::time::Instant::now();
            let d = deltas_to_speech(&speech, &cues);
            let t_align = t1.elapsed();
            apply(&mut cues, &d);
            let mut signed: Vec<i64> = cues
                .iter()
                .zip(&expect)
                .filter_map(|(c, t)| t.map(|t| c.start_ms - t))
                .collect();
            signed.sort_unstable();
            let mut err: Vec<i64> = signed.iter().map(|e| e.abs()).collect();
            err.sort_unstable();
            let median = err[err.len() / 2];
            let p90 = err[err.len() * 9 / 10];
            eprintln!(
                "{case}: {} lines, {} speech spans, error median {median} ms (signed {} ms), p90 {p90} ms, max {} ms \
                 (decode {t_decode:?}, vad {t_vad:?}, align {t_align:?})",
                cues.len(),
                speech.spans.len(),
                signed[signed.len() / 2],
                err.last().unwrap()
            );
            // The sample's subs start LEAD_MS (100 ms) before each voice, like synced lines, so the
            // error here is how far from the real voice onsets lines land. Correct subs don't move.
            if case == "exact" {
                assert!(
                    d.iter().all(|x| *x == 0),
                    "exact: moved {:?}",
                    summarize(&d)
                );
            }
            let max = *err.last().unwrap();
            assert!(
                median <= 10 && max <= 15,
                "{case}: median {median} ms, max {max} ms"
            );

            // English from another release follows the synced Japanese: line i under line i.
            let en_bytes = std::fs::read(dir.join(case).join("Ep.en.srt")).unwrap();
            let mut en = subtitles::parse(&en_bytes, None).unwrap().cues;
            let f = deltas_follow(&cues, &en);
            apply(&mut en, &f);
            // English line k translates the line whose true start is starts[k]: each spoken
            // Japanese cue must pair with exactly that English line (lyrics/signs: no check).
            let pairs = subtitles::align(&cues, &en);
            let mut checked = 0;
            let wrong = pairs
                .iter()
                .filter(|(j, e)| {
                    let Some(Some(t)) = expect.get(*j as usize) else {
                        return false;
                    };
                    checked += 1;
                    let k = starts.iter().position(|s| s == t).unwrap() as u32;
                    e.as_slice() != [k]
                })
                .count();
            eprintln!("{case} English: {wrong} of {checked} spoken lines paired wrong");
            assert_eq!(
                wrong, 0,
                "{case}: English lines under the wrong Japanese line"
            );
            if case == "exact" {
                assert!(f.iter().all(|x| *x == 0), "exact English moved");
            }
        }
    }
}
