//! Lookup speed with a large user frequency dictionary attached, against the real database.
//! `cargo test -q --release -p jpdict --test yomitan_bench -- --ignored --nocapture`
//! (set JPDICT_DB to the jmdict.sqlite when it is not at src-tauri/lib/dict/).
mod common;

use std::path::PathBuf;
use std::time::Instant;

use common::temp_dir;
use common::zipgen::{index_json, write, zip_of};
use jpdict::Dict;
use serde_json::json;

const LINES: [&str; 6] = [
    "夕焼けがこんなに綺麗だなんて知らなかった",
    "食べさせられなかったのに、彼は諦めない。",
    "忘れないでね、持ってきたよ",
    "あの人が撮ろうと言った写真を見に行こう",
    "勉強していたから、すっげーケーキを食べちゃった",
    "ありがとう、会おうよ。また明日",
];

fn db_path() -> PathBuf {
    std::env::var_os("JPDICT_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../src-tauri/lib/dict/jmdict.sqlite"
            ))
        })
}

fn measure(d: &Dict) -> (f64, f64, f64, usize) {
    let mut samples: Vec<f64> = Vec::new();
    for _round in 0..5 {
        for line in LINES {
            for off in 0..line.chars().count() {
                let t = Instant::now();
                let r = d.lookup_at(line, off);
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
                std::hint::black_box(r);
            }
        }
    }
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    (samples[n / 2], samples[n * 95 / 100], samples[n - 1], n)
}

#[test]
#[ignore = "needs the real database"]
fn lookup_speed_with_user_dictionaries_attached() {
    let path = db_path();
    let d = Dict::open(&path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let (m50, m95, mmax, n) = measure(&d);
    println!("baseline ({n} lookups): p50 {m50:.3} ms, p95 {m95:.3} ms, max {mmax:.2} ms");

    // ~150k frequency rows over real forms, plus 30k synthetic terms (half of them on real readings).
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut stmt = conn
        .prepare("SELECT text FROM forms WHERE kind = 0 ORDER BY entry_id LIMIT 150000")
        .unwrap();
    let words: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(words.len() > 100_000, "only {} forms", words.len());
    let freq: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, w)| json!([w, "freq", {"value": (i * 7919 % 150_000) + 1, "displayValue": "x"}]))
        .collect();
    let terms: Vec<_> = (0..30_000)
        .map(|i| {
            let w = &words[(i * 5) % words.len()];
            json!([
                format!("{w}{}", i % 3),
                "",
                "n",
                "",
                i,
                [format!("synthetic {i}")],
                i,
                ""
            ])
        })
        .collect();
    let dir = temp_dir("yomitan-bench");
    let z = zip_of(&[
        ("index.json", index_json("Bench", "1", json!({}))),
        ("term_bank_1.json", serde_json::to_vec(&terms).unwrap()),
        ("term_meta_bank_1.json", serde_json::to_vec(&freq).unwrap()),
    ]);
    let user = dir.join("user.sqlite");
    let t = Instant::now();
    let s = d.import_yomitan(&write(&dir, "b.zip", &z), &user).unwrap();
    println!(
        "import: {} terms + {} freq rows from a {:.1} MB zip in {:.2} s; user db {:.1} MB",
        s.terms,
        s.frequencies,
        z.len() as f64 / 1e6,
        t.elapsed().as_secs_f64(),
        std::fs::metadata(&user).unwrap().len() as f64 / 1e6
    );

    let (a50, a95, amax, n) = measure(&d);
    println!("with user db ({n} lookups): p50 {a50:.3} ms, p95 {a95:.3} ms, max {amax:.2} ms");
    assert!(a95 < 2.0, "p95 {a95:.3} ms over the 2 ms budget");
    let hit = d.lookup_at("食べさせられなかった", 0);
    println!(
        "sample: {:?}",
        hit.iter()
            .flat_map(|r| &r.entries)
            .take(3)
            .map(|e| (e.term.as_str(), e.freq_rank))
            .collect::<Vec<_>>()
    );
}
