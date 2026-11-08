//! Seeded mutation fuzz: import thousands of damaged Yomitan zips; none may panic, hang, or leave
//! half an import behind.
mod common;

use std::time::{Duration, Instant};

use common::zipgen::{freq_zip, index_json, term_zip, write, zip_of};
use common::{fixture_dict, temp_dir};
use serde_json::json;

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

const SNIPPETS: [&[u8]; 9] = [
    b"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[",
    b"{\"type\":\"structured-content\",\"content\":",
    b"99999999999999999999999",
    b"\"freq\"",
    b"\"pitch\"",
    b"null,null,",
    b"-1e999",
    b"\xff\xfe\x00",
    b"\"\\ud800\"",
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
                if !b.is_empty() {
                    let from = rng.below(b.len());
                    let to = (from + 1 + rng.below(40)).min(b.len());
                    let chunk = b[from..to].to_vec();
                    let at = rng.below(b.len() + 1);
                    b.splice(at..at, chunk);
                }
            }
            3 => {
                let s = SNIPPETS[rng.below(SNIPPETS.len())];
                let i = rng.below(b.len() + 1);
                b.splice(i..i, s.iter().copied());
            }
            4 => {
                if !b.is_empty() {
                    let from = rng.below(b.len());
                    let to = (from + 1 + rng.below(20)).min(b.len());
                    b.drain(from..to);
                }
            }
            _ => {
                let i = rng.below(b.len() + 1);
                b.insert(i, rng.below(256) as u8);
            }
        }
    }
    b
}

/// Valid member contents: (index.json, term_bank_1.json, term_meta_bank_1.json).
fn seeds() -> Vec<[Vec<u8>; 3]> {
    let terms = json!([
        ["食べる", "たべる", "v1 vt", "v1", 5, ["to eat", {"type": "structured-content", "content": [{"tag": "ul", "content": [{"tag": "li", "content": "x"}]}]}], 1, "common"],
        ["沈丁花", "じんちょうげ", "n", "", 0, ["daphne"], 2, ""],
        ["勉強", "", "n vs", "vs", -3, ["study"], 3, ""]
    ]);
    let meta = json!([
        ["夕焼け", "freq", {"reading": "ゆうやけ", "frequency": {"value": 5000, "displayValue": "5000"}}],
        ["夕", "freq", 100],
        ["沈丁花", "freq", {"value": 7, "displayValue": "7"}],
        ["沈丁花", "pitch", {"reading": "じんちょうげ", "pitches": [{"position": 3}, {"position": [1, 2]}]}]
    ]);
    let idx = index_json("Fuzz", "1", json!({"frequencyMode": "occurrence-based"}));
    vec![[
        idx,
        serde_json::to_vec(&terms).unwrap(),
        serde_json::to_vec(&meta).unwrap(),
    ]]
}

#[test]
fn damaged_zips_never_panic_or_corrupt() {
    let dir = temp_dir("yomitan-fuzz");
    let d = fixture_dict("yomitan-fuzz");
    let user = dir.join("user.sqlite");
    let keep = term_zip(
        "Keep",
        &[("沈丁花", "じんちょうげ", "n", "", 0, json!(["daphne"]))],
    );
    d.import_yomitan(&write(&dir, "keep.zip", &keep), &user)
        .unwrap();
    let _ = freq_zip("unused", &[], json!({}));
    let baseline = d.user_dictionaries();
    let seeds = seeds();
    let zip_path = dir.join("f.zip");

    let started = Instant::now();
    let (mut ok, mut failed) = (0u32, 0u32);
    for seed in 1..=500u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let s = &seeds[rng.below(seeds.len())];
        let bytes = if rng.below(2) == 0 {
            // Damage the JSON inside a well-formed zip (so the importer sees it, not just a bad CRC).
            let mut parts = s.clone();
            let i = rng.below(3);
            if rng.below(3) != 0 {
                parts[i] = mutate(&parts[i], &mut rng);
            }
            zip_of(&[
                ("index.json", parts[0].clone()),
                ("term_bank_1.json", parts[1].clone()),
                ("term_meta_bank_1.json", parts[2].clone()),
            ])
        } else {
            // Damage the zip container itself.
            let whole = zip_of(&[
                ("index.json", s[0].clone()),
                ("term_bank_1.json", s[1].clone()),
                ("term_meta_bank_1.json", s[2].clone()),
            ]);
            mutate(&whole, &mut rng)
        };
        std::fs::write(&zip_path, &bytes).unwrap();
        let before = d.user_dictionaries();
        let t = Instant::now();
        let r = d.import_yomitan(&zip_path, &user);
        let took = t.elapsed();
        // Linear budget (debug build, generous: a commit can stall on fsync): 2 s + 1 ms per KB.
        let budget = Duration::from_secs(2) + Duration::from_millis(bytes.len() as u64 / 1024);
        assert!(took < budget, "import took {took:?}, seed {seed}");
        match r {
            Ok(_) => {
                ok += 1;
                // Lookups keep working on whatever was imported.
                let _ = d.lookup_at("沈丁花食べる勉強", 0);
                let _ = d.lookup_at("夕焼け", 0);
                // Put the baseline back for the next round.
                d.remove_user_dictionary("Fuzz").ok();
                for (title, ..) in d.user_dictionaries() {
                    if title != "Keep" {
                        d.remove_user_dictionary(&title).unwrap();
                    }
                }
            }
            Err(_) => {
                failed += 1;
                assert_eq!(
                    d.user_dictionaries(),
                    before,
                    "failed import changed data, seed {seed}"
                );
            }
        }
        if started.elapsed() > Duration::from_secs(120) {
            panic!("fuzz run too slow at seed {seed}");
        }
    }
    assert_eq!(d.user_dictionaries(), baseline);
    assert!(
        ok > 50 && failed > 50,
        "ok {ok}, failed {failed}: the mutations are not varied enough"
    );
}
