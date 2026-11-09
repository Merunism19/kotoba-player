//! Seeded mutation fuzz: parse thousands of damaged files; none may panic or be slow.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use subtitles::parse;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

fn fixtures() -> Vec<Vec<u8>> {
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut v: Vec<Vec<u8>> = ["anime.ass", "jp.ass", "sample.srt", "en.srt", "sample.vtt"]
        .iter()
        .map(|n| std::fs::read(format!("{dir}/{n}")).unwrap())
        .collect();
    let crlf = String::from_utf8(v[2].clone())
        .unwrap()
        .replace('\n', "\r\n");
    v.push(crlf.into_bytes());
    let (sjis, _, _) =
        encoding_rs::SHIFT_JIS.encode("1\n00:00:01,000 --> 00:00:02,000\nこんにちは\n");
    v.push(sjis.into_owned());
    v
}

const SNIPPETS: [&str; 8] = [
    "99999999999999999999999",
    "{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{",
    "<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<",
    "\\pos(\\k\\p1\\an9\\move(",
    "9:99:99.99,9:99:99.99,",
    "-->",
    "\\N\\N\\N\\h\\h",
    "Dialogue: ",
];

fn mutate(src: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = src.to_vec();
    for _ in 0..=rng.below(4) {
        match rng.below(6) {
            0 => {
                for _ in 0..=rng.below(8) {
                    if !b.is_empty() {
                        let i = rng.below(b.len());
                        b[i] ^= 1 << rng.below(8);
                    }
                }
            }
            1 => b.truncate(rng.below(b.len() + 1)),
            2 => {
                // duplicate a run of lines
                let text = String::from_utf8_lossy(&b).into_owned();
                let lines: Vec<&str> = text.lines().collect();
                if !lines.is_empty() {
                    let from = rng.below(lines.len());
                    let to = (from + 1 + rng.below(10)).min(lines.len());
                    let mut out: Vec<&str> = lines.clone();
                    for _ in 0..=rng.below(20) {
                        out.splice(from..from, lines[from..to].iter().copied());
                    }
                    b = out.join("\n").into_bytes();
                }
            }
            3 | 4 => {
                let s = SNIPPETS[rng.below(SNIPPETS.len())];
                let i = rng.below(b.len() + 1);
                let times = if rng.below(3) == 0 { 200 } else { 1 };
                let ins: Vec<u8> = s.as_bytes().repeat(times);
                b.splice(i..i, ins);
            }
            _ => {
                let i = rng.below(b.len() + 1);
                let junk: Vec<u8> = (0..=rng.below(16)).map(|_| rng.next() as u8).collect();
                b.splice(i..i, junk);
            }
        }
    }
    b
}

#[test]
fn mutation_fuzz() {
    let fixtures = fixtures();
    let batch = Instant::now();
    for seed in 1..=2_000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let src = &fixtures[rng.below(fixtures.len())];
        let input = mutate(src, &mut rng);
        let t = Instant::now();
        let r = std::panic::catch_unwind(|| parse(&input, None));
        let took = t.elapsed();
        assert!(r.is_ok(), "parse panicked, seed {seed}");
        assert!(
            took < Duration::from_millis(50),
            "parse took {took:?}, seed {seed}"
        );
        if let Ok(Ok(track)) = r {
            let _ = track.signs_only_ass();
            for (i, c) in track.cues.iter().enumerate() {
                assert_eq!(c.id as usize, i, "bad id, seed {seed}");
            }
        }
    }
    assert!(
        batch.elapsed() < Duration::from_secs(10),
        "batch took {:?}",
        batch.elapsed()
    );
}
