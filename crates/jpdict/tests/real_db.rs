//! Runs against the real database built by `node scripts/fetch-deps.mjs`.
//! `cargo test -p jpdict --release --test real_db -- --ignored --nocapture`

use std::path::PathBuf;
use std::time::Instant;

use jpdict::Dict;

fn real_db() -> Dict {
    let path = std::env::var_os("JPDICT_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../src-tauri/lib/dict/jmdict.sqlite"
            ))
        });
    Dict::open(&path).unwrap_or_else(|e| {
        panic!(
            "open {}: {e} (run node scripts/fetch-deps.mjs)",
            path.display()
        )
    })
}

const LINES: [&str; 6] = [
    "夕焼けがこんなに綺麗だなんて知らなかった",
    "食べさせられなかったのに、彼は諦めない。",
    "忘れないでね、持ってきたよ",
    "あの人が撮ろうと言った写真を見に行こう",
    "勉強していたから、すっげーケーキを食べちゃった",
    "ありがとう、会おうよ。また明日",
];

#[test]
#[ignore = "needs the real database"]
fn real_samples() {
    let d = real_db();
    println!(
        "meta: jmdict {} ({})",
        d.meta("jmdict_version").unwrap(),
        d.meta("jmdict_date").unwrap()
    );
    for (text, off) in [
        ("夕焼けがこんなに綺麗だなんて知らなかった", 0),
        ("夕焼けがこんなに綺麗だなんて知らなかった", 8),
        ("夕焼けがこんなに綺麗だなんて知らなかった", 14),
        ("諦めない", 0),
        ("食べさせられなかった", 0),
        ("行こう", 0),
        ("撮ろうと言った", 0),
        ("会おうよ", 0),
        ("持ってきた", 0),
        ("ケーキ", 0),
        ("お願いします", 0),
        ("言っちゃった", 0),
        ("勉強していた", 0),
        ("忘れないで", 0),
        ("高くなかった", 0),
    ] {
        let r = d.lookup_at(text, off);
        let Some(top) = r.first() else {
            println!("{text}@{off}: NO RESULT");
            continue;
        };
        let e = &top.entries[0];
        let gloss: Vec<&str> = e.senses[0]
            .glosses
            .iter()
            .take(3)
            .map(String::as_str)
            .collect();
        let pitch: Vec<String> = e
            .pitch
            .iter()
            .map(|p| format!("{}:{}", p.downstep, p.pattern))
            .collect();
        let ruby: String = e
            .furigana
            .iter()
            .map(|f| {
                f.ruby
                    .as_ref()
                    .map_or(f.text.clone(), |r| format!("{}[{}]", f.text, r))
            })
            .collect();
        println!(
            "{text}@{off} -> matched {:?} ({} results) deinflected {:?} {:?} | {} / {} / {} | {} | pitch {:?} | common {}",
            top.matched,
            r.len(),
            top.deinflected,
            top.inflections,
            e.term,
            e.reading,
            ruby,
            gloss.join("; "),
            pitch,
            e.common
        );
    }
    let r = d.lookup_at("夕焼けがこんなに綺麗だなんて知らなかった", 0);
    assert_eq!(r[0].matched, "夕焼け");
}

#[test]
#[ignore = "needs the real database; run with --release"]
fn bench_lookup_at() {
    let d = real_db();
    // Warm up the page cache.
    for l in LINES {
        let _ = d.lookup_at(l, 0);
    }
    let mut samples: Vec<f64> = Vec::new();
    for l in LINES {
        for off in 0..l.chars().count() {
            let t = Instant::now();
            let r = d.lookup_at(l, off);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(r);
        }
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = samples.len();
    let mean = samples.iter().sum::<f64>() / n as f64;
    println!(
        "lookup_at over {n} click positions: mean {mean:.2} ms, median {:.2} ms, p95 {:.2} ms, max {:.2} ms",
        samples[n / 2],
        samples[n * 95 / 100],
        samples[n - 1]
    );
    assert!(samples[n * 95 / 100] < 5.0, "p95 over budget");
}
