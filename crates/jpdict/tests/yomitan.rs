mod common;

use common::zipgen::{freq_zip, index_json, term_zip, write, zip_of};
use common::{ACCENTS, fixture_dict, fixture_json, temp_dir};
use jpdict::{Dict, ImportKind};
use serde_json::{Value, json};

fn glosses(e: &jpdict::DictEntry) -> Vec<String> {
    e.senses.iter().flat_map(|s| s.glosses.clone()).collect()
}

#[test]
fn terms_and_frequencies_show_up_in_lookups() {
    let dir = temp_dir("yomitan-basic");
    let d = fixture_dict("yomitan-basic");
    let user = dir.join("user.sqlite");
    // Before any import nothing changes.
    assert!(d.lookup_at("夕焼け", 0)[0].entries[0].freq_rank.is_none());
    assert!(d.user_dictionaries().is_empty());

    let terms = term_zip(
        "My Dict",
        &[
            (
                "沈丁花",
                "じんちょうげ",
                "n",
                "",
                10,
                json!([
                    "daphne",
                    {"type": "structured-content", "content": [
                        {"tag": "ul", "content": [
                            {"tag": "li", "content": "winter daphne"},
                            {"tag": "li", "content": [{"tag": "ruby", "content": ["漢", {"tag": "rt", "content": "かん"}]}, " flower"]},
                            {"tag": "img", "path": "x.png"}
                        ]}
                    ]}
                ]),
            ),
            (
                "食べる",
                "たべる",
                "v1",
                "v1",
                5,
                json!(["to eat (user copy)"]),
            ),
            ("食べる", "たべる", "v1 vt", "v1", 5, json!(["to eat"])),
            (
                "勉強",
                "べんきょう",
                "n vs",
                "vs",
                3,
                json!(["study; work"]),
            ),
        ],
    );
    let freq = freq_zip(
        "Freq A",
        &[
            (
                "夕焼け",
                json!({"reading": "ゆうやけ", "frequency": {"value": 5000, "displayValue": "5000x"}}),
            ),
            ("夕", json!(100)),
            ("沈丁花", json!({"value": 777, "displayValue": "777"})),
            ("食べる", json!(30)),
        ],
        json!({"frequencyMode": "rank-based"}),
    );
    let t = d
        .import_yomitan(&write(&dir, "t.zip", &terms), &user)
        .unwrap();
    assert_eq!(
        (t.title.as_str(), t.kind, t.terms, t.frequencies),
        ("My Dict", ImportKind::Terms, 4, 0)
    );
    let f = d
        .import_yomitan(&write(&dir, "f.zip", &freq), &user)
        .unwrap();
    assert_eq!(
        (f.kind, f.terms, f.frequencies, f.revision.as_str()),
        (ImportKind::Frequency, 0, 4, "2024.1")
    );
    assert_eq!(
        d.user_dictionaries(),
        [
            ("My Dict".to_string(), ImportKind::Terms, 4),
            ("Freq A".to_string(), ImportKind::Frequency, 4)
        ]
    );

    // A word only the user dictionary knows.
    let r = d.lookup_at("沈丁花が咲く", 0);
    assert_eq!(r[0].matched, "沈丁花");
    let e = &r[0].entries[0];
    assert!(e.id < 0);
    assert_eq!(
        (e.term.as_str(), e.reading.as_str()),
        ("沈丁花", "じんちょうげ")
    );
    assert_eq!(
        e.senses[0].glosses,
        ["daphne", "winter daphne", "漢 flower"]
    );
    assert_eq!(e.senses[0].pos, ["noun"]);
    assert_eq!(e.freq_rank, Some(777));
    // Frequency attaches to JMdict entries by (term, reading).
    let e = &d.lookup_at("夕焼け", 0)[0].entries[0];
    assert_eq!((e.id, e.freq_rank), (1542720, Some(5000)));
    // JMdict entry first; the user copy with identical glosses is dropped, the different one kept.
    let r = d.lookup_at("食べる", 0);
    let ids: Vec<i64> = r[0].entries.iter().map(|e| e.id).collect();
    assert!(ids[0] > 0 && ids[1..].iter().all(|i| *i < 0), "{ids:?}");
    assert_eq!(r[0].entries[0].freq_rank, Some(30));
    let all: Vec<Vec<String>> = r[0].entries.iter().map(glosses).collect();
    assert!(all.contains(&vec!["to eat (user copy)".to_string()]));
    assert_eq!(
        all.iter().filter(|g| **g == ["to eat".to_string()]).count(),
        1
    );
    // Deinflection works for user verbs when the rules agree.
    let r = d.lookup_at("食べた", 0);
    assert_eq!(r[0].inflections, ["past"]);
    assert!(r[0].entries.iter().any(|e| e.id < 0));
    // A suru-noun from a user dictionary.
    let r = d.lookup_at("勉強した", 0);
    assert!(
        r.iter()
            .flat_map(|x| &x.entries)
            .any(|e| e.term == "勉強" && e.id < 0)
    );
    // Shorter results do not repeat entries found in a longer match.
    let r = d.lookup_at("沈丁花", 0);
    assert_eq!(
        r.iter()
            .flat_map(|x| &x.entries)
            .filter(|e| e.term == "沈丁花")
            .count(),
        1
    );
}

fn ids_for(d: &Dict, text: &str) -> Vec<i64> {
    d.lookup_at(text, 0)[0]
        .entries
        .iter()
        .map(|e| e.id)
        .collect()
}

#[test]
fn frequency_reorders_equally_ranked_entries() {
    let dir = temp_dir("yomitan-rank");
    let mut v: Value = serde_json::from_str(&fixture_json()).unwrap();
    let mk = |id: &str, kanji: &str, gloss: &str| {
        json!({"id": id, "kanji": [{"common": false, "text": kanji, "tags": []}],
               "kana": [{"common": false, "text": "はし", "tags": [], "appliesToKanji": ["*"]}],
               "sense": [{"partOfSpeech": ["n"], "appliesToKanji": ["*"], "appliesToKana": ["*"], "related": [], "antonym": [],
                          "field": [], "dialect": [], "misc": [], "info": [], "languageSource": [],
                          "gloss": [{"lang": "eng", "gender": null, "type": null, "text": gloss}]}]})
    };
    let words = v["words"].as_array_mut().unwrap();
    words.push(mk("1900001", "橋", "bridge"));
    words.push(mk("1900002", "箸", "chopsticks"));
    words.push(mk("1900003", "端", "edge"));
    let json_path = write(
        &dir,
        "j.json",
        serde_json::to_string(&v).unwrap().as_bytes(),
    );
    let acc = write(&dir, "a.txt", ACCENTS.as_bytes());
    let db = dir.join("d.sqlite");
    jpdict::build::build_dict(&json_path, &acc, &db).unwrap();
    let d = Dict::open(&db).unwrap();
    assert_eq!(ids_for(&d, "はし"), [1900001, 1900002, 1900003]);

    let user = dir.join("user.sqlite");
    let freq = freq_zip(
        "Freq",
        &[("橋", json!(9000)), ("箸", json!(100)), ("端", json!(50))],
        json!({}),
    );
    d.import_yomitan(&write(&dir, "f.zip", &freq), &user)
        .unwrap();
    assert_eq!(ids_for(&d, "はし"), [1900003, 1900002, 1900001]);
    let ranks: Vec<_> = d.lookup_at("はし", 0)[0]
        .entries
        .iter()
        .map(|e| e.freq_rank)
        .collect();
    assert_eq!(ranks, [Some(50), Some(100), Some(9000)]);

    // Best rank over several dictionaries; occurrence counts become ranks (highest count = 1).
    let occ = freq_zip(
        "Counts",
        &[("橋", json!(5000)), ("箸", json!(10)), ("端", json!(70))],
        json!({"frequencyMode": "occurrence-based"}),
    );
    let s = d
        .import_yomitan(&write(&dir, "o.zip", &occ), &user)
        .unwrap();
    assert_eq!(s.frequencies, 3);
    let got: Vec<(i64, Option<u32>)> = d.lookup_at("はし", 0)[0]
        .entries
        .iter()
        .map(|e| (e.id, e.freq_rank))
        .collect();
    assert_eq!(
        got,
        [(1900001, Some(1)), (1900003, Some(2)), (1900002, Some(3))]
    );
}

#[test]
fn user_entries_with_equal_reading_are_ranked_by_frequency() {
    let dir = temp_dir("yomitan-userrank");
    let d = fixture_dict("yomitan-userrank");
    let user = dir.join("user.sqlite");
    let terms = term_zip(
        "T",
        &[
            ("描く", "かく", "v5", "v5", 9, json!(["to draw"])),
            ("掻く", "かく", "v5", "v5", 1, json!(["to scratch"])),
        ],
    );
    let freq = freq_zip(
        "F",
        &[("描く", json!(40)), ("掻く", json!(4000))],
        json!({}),
    );
    d.import_yomitan(&write(&dir, "t.zip", &terms), &user)
        .unwrap();
    // No frequency yet: score decides.
    let t: Vec<String> = d.lookup_at("かく", 0)[0]
        .entries
        .iter()
        .map(|e| e.term.clone())
        .collect();
    assert_eq!(t, ["書く", "描く", "掻く"]);
    d.import_yomitan(&write(&dir, "f.zip", &freq), &user)
        .unwrap();
    let t: Vec<String> = d.lookup_at("かく", 0)[0]
        .entries
        .iter()
        .map(|e| e.term.clone())
        .collect();
    assert_eq!(t, ["書く", "描く", "掻く"]);
    let freq2 = freq_zip(
        "F",
        &[("描く", json!(9000)), ("掻く", json!(10))],
        json!({}),
    );
    d.import_yomitan(&write(&dir, "f2.zip", &freq2), &user)
        .unwrap();
    let t: Vec<String> = d.lookup_at("かく", 0)[0]
        .entries
        .iter()
        .map(|e| e.term.clone())
        .collect();
    assert_eq!(t, ["書く", "掻く", "描く"]);
}

#[test]
fn reimport_replaces_and_remove_works() {
    let dir = temp_dir("yomitan-replace");
    let d = fixture_dict("yomitan-replace");
    let user = dir.join("sub").join("user.sqlite");
    let v1 = term_zip(
        "D",
        &[("沈丁花", "じんちょうげ", "n", "", 0, json!(["daphne"]))],
    );
    let v2 = term_zip(
        "D",
        &[("紫陽花", "あじさい", "n", "", 0, json!(["hydrangea"]))],
    );
    d.import_yomitan(&write(&dir, "1.zip", &v1), &user).unwrap();
    assert!(!d.lookup_at("沈丁花", 0).is_empty());
    d.import_yomitan(&write(&dir, "2.zip", &v2), &user).unwrap();
    assert!(d.lookup_at("沈丁花", 0).is_empty());
    assert_eq!(
        d.lookup_at("紫陽花", 0)[0].entries[0].senses[0].glosses,
        ["hydrangea"]
    );
    assert_eq!(
        d.user_dictionaries(),
        [("D".to_string(), ImportKind::Terms, 1)]
    );

    d.remove_user_dictionary("D").unwrap();
    assert!(d.lookup_at("紫陽花", 0).is_empty());
    assert!(d.user_dictionaries().is_empty());
    d.remove_user_dictionary("nope").unwrap();

    // Attaching an existing file at startup (a second Dict) sees the data.
    d.import_yomitan(&write(&dir, "2.zip", &v2), &user).unwrap();
    let d2 = fixture_dict("yomitan-replace2");
    d2.attach_user_db(&user).unwrap();
    assert_eq!(d2.lookup_at("紫陽花", 0)[0].entries[0].term, "紫陽花");
    assert_eq!(d2.user_dictionaries().len(), 1);
}

#[test]
fn pitch_entries_are_imported() {
    let dir = temp_dir("yomitan-pitch");
    let d = fixture_dict("yomitan-pitch");
    let user = dir.join("user.sqlite");
    let bank = json!([["沈丁花", "pitch", {"reading": "じんちょうげ", "pitches": [{"position": 3}, {"position": 0}]}]]);
    let z = zip_of(&[
        ("index.json", index_json("Pitch", "1", json!({}))),
        ("term_meta_bank_1.json", serde_json::to_vec(&bank).unwrap()),
    ]);
    let terms = term_zip(
        "T",
        &[("沈丁花", "じんちょうげ", "n", "", 0, json!(["daphne"]))],
    );
    let s = d.import_yomitan(&write(&dir, "p.zip", &z), &user).unwrap();
    assert_eq!((s.kind, s.pitches), (ImportKind::Pitch, 1));
    d.import_yomitan(&write(&dir, "t.zip", &terms), &user)
        .unwrap();
    let e = &d.lookup_at("沈丁花", 0)[0].entries[0];
    assert_eq!(
        e.pitch.iter().map(|p| p.downstep).collect::<Vec<_>>(),
        [3, 0]
    );
}

#[test]
fn hostile_and_broken_zips_are_errors_not_panics() {
    let dir = temp_dir("yomitan-hostile");
    let d = fixture_dict("yomitan-hostile");
    let user = dir.join("user.sqlite");
    let idx = index_json("X", "1", json!({}));
    let row = serde_json::to_vec(&json!([["a", "b", "", "", 0, ["g"], 1, ""]])).unwrap();
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000)).into_bytes();
    let deep_row = format!(
        "[[\"a\",\"b\",\"\",\"\",0,{}{}]]",
        "[".repeat(5000),
        "]".repeat(5000)
    )
    .into_bytes();
    let with =
        |name: &str, data: &[u8]| zip_of(&[("index.json", idx.clone()), (name, data.to_vec())]);

    let mut cases: Vec<(&str, Vec<u8>)> = vec![
        ("not a zip", b"hello, definitely not a zip".to_vec()),
        ("empty", Vec::new()),
        ("no index", zip_of(&[("term_bank_1.json", row.clone())])),
        ("index not json", zip_of(&[("index.json", b"{{{".to_vec())])),
        (
            "index wrong type",
            zip_of(&[("index.json", b"[1,2]".to_vec())]),
        ),
        (
            "index no title",
            zip_of(&[("index.json", br#"{"format":3}"#.to_vec())]),
        ),
        (
            "index title type",
            zip_of(&[("index.json", br#"{"title":5,"format":3}"#.to_vec())]),
        ),
        (
            "index bad format",
            zip_of(&[("index.json", br#"{"title":"x","format":1}"#.to_vec())]),
        ),
        (
            "index no format",
            zip_of(&[("index.json", br#"{"title":"x"}"#.to_vec())]),
        ),
        ("bank not json", with("term_bank_1.json", b"nope")),
        ("bank not array", with("term_bank_1.json", b"{\"a\":1}")),
        ("bank trailing junk", with("term_bank_1.json", b"[] x")),
        (
            "bank truncated",
            with("term_bank_1.json", b"[[\"a\",\"b\",\"\",\"\",0,[\"g\"]"),
        ),
        ("row not array", with("term_bank_1.json", b"[5]")),
        ("row too short", with("term_bank_1.json", b"[[\"a\"]]")),
        (
            "row wrong types",
            with("term_bank_1.json", b"[[1,2,3,4,5,6]]"),
        ),
        (
            "glossary not array",
            with("term_bank_1.json", b"[[\"a\",\"b\",\"\",\"\",0,\"g\"]]"),
        ),
        (
            "score not number",
            with(
                "term_bank_1.json",
                b"[[\"a\",\"b\",\"\",\"\",\"x\",[\"g\"]]]",
            ),
        ),
        (
            "meta wrong types",
            with("term_meta_bank_1.json", b"[[\"a\",\"freq\",\"x\"]]"),
        ),
        (
            "meta bad pitch",
            with("term_meta_bank_1.json", b"[[\"a\",\"pitch\",5]]"),
        ),
        ("deep array", with("term_bank_1.json", &deep)),
        ("deep row", with("term_bank_1.json", &deep_row)),
        ("nothing usable", with("term_bank_1.json", b"[]")),
        ("zip slip", with("../../evil.json", b"[]")),
        ("zip slip abs", with("/etc/passwd", b"[]")),
        ("zip slip drive", with("C:/evil.json", b"[]")),
        ("zip slip backslash", with("a\\..\\evil", b"[]")),
    ];
    // A member whose declared size is huge: patch the size fields in both the local and central headers.
    let mut big = with("term_bank_1.json", &row);
    for sig in [[0x50u8, 0x4b, 0x01, 0x02], [0x50, 0x4b, 0x03, 0x04]] {
        let mut from = 0;
        while let Some(p) = big[from..]
            .windows(4)
            .position(|w| w == sig)
            .map(|p| p + from)
        {
            let off = if sig[2] == 1 { p + 24 } else { p + 22 };
            if off + 4 <= big.len() {
                big[off..off + 4].copy_from_slice(&0xF000_0000u32.to_le_bytes());
            }
            from = p + 4;
        }
    }
    cases.push(("huge declared size", big));

    // The user data must be untouched by every failure.
    let ok = term_zip(
        "Keep",
        &[("沈丁花", "じんちょうげ", "n", "", 0, json!(["daphne"]))],
    );
    d.import_yomitan(&write(&dir, "ok.zip", &ok), &user)
        .unwrap();
    for (name, bytes) in &cases {
        let p = write(&dir, "case.zip", bytes);
        let r = d.import_yomitan(&p, &user);
        assert!(r.is_err(), "{name} should fail, got {r:?}");
        assert_eq!(
            d.user_dictionaries(),
            [("Keep".to_string(), ImportKind::Terms, 1)],
            "{name}"
        );
    }
    assert!(d.import_yomitan(&dir.join("missing.zip"), &user).is_err());
    assert_eq!(
        d.lookup_at("沈丁花", 0)[0].entries[0].senses[0].glosses,
        ["daphne"]
    );
}

#[test]
fn a_failed_reimport_keeps_the_old_data() {
    let dir = temp_dir("yomitan-atomic");
    let d = fixture_dict("yomitan-atomic");
    let user = dir.join("user.sqlite");
    let v1 = term_zip(
        "D",
        &[("沈丁花", "じんちょうげ", "n", "", 0, json!(["daphne"]))],
    );
    d.import_yomitan(&write(&dir, "1.zip", &v1), &user).unwrap();
    // Good first bank, broken second bank: nothing of the new version may remain.
    let good = serde_json::to_vec(&json!([[
        "紫陽花",
        "あじさい",
        "",
        "",
        0,
        ["hydrangea"],
        1,
        ""
    ]]))
    .unwrap();
    let bad = zip_of(&[
        ("index.json", index_json("D", "2", json!({}))),
        ("term_bank_1.json", good),
        ("term_bank_2.json", b"[[1]]".to_vec()),
    ]);
    assert!(
        d.import_yomitan(&write(&dir, "2.zip", &bad), &user)
            .is_err()
    );
    assert!(d.lookup_at("紫陽花", 0).is_empty());
    assert_eq!(
        d.lookup_at("沈丁花", 0)[0].entries[0].senses[0].glosses,
        ["daphne"]
    );
}
