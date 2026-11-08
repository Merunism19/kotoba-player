mod common;

use common::{fixture_dict, offset_of};
use jpdict::{Dict, LookupResult};
use serde_json::Value;
use std::sync::OnceLock;

fn dict() -> &'static Dict {
    static D: OnceLock<Dict> = OnceLock::new();
    D.get_or_init(|| fixture_dict("lookup"))
}

const LINE: &str = "夕焼けがこんなに綺麗だなんて知らなかった";

fn lookup(text: &str, needle: &str) -> Vec<LookupResult> {
    dict().lookup_at(text, offset_of(text, needle))
}

fn terms(r: &LookupResult) -> Vec<&str> {
    r.entries.iter().map(|e| e.term.as_str()).collect()
}

#[test]
fn dict_is_send_and_sync() {
    fn assert_ss<T: Send + Sync>() {}
    assert_ss::<Dict>();
}

#[test]
fn click_on_yuuyake_finds_the_whole_word_not_one_kanji() {
    let r = dict().lookup_at(LINE, 0);
    assert_eq!(r[0].matched, "夕焼け");
    assert_eq!((r[0].start, r[0].len), (0, 3));
    assert_eq!(terms(&r[0]), ["夕焼け"]);
    assert_eq!(r[0].entries[0].reading, "ゆうやけ");
    assert!(r[0].entries[0].common);
    assert!(r[0].deinflected.is_none() && r[0].inflections.is_empty());
    // The single kanji is still offered as a shorter result, and the longer entry is not repeated in it.
    let last = r.last().unwrap();
    assert_eq!((last.matched.as_str(), terms(last)), ("夕", vec!["夕"]));
}

#[test]
fn furigana_and_pitch_come_with_the_entry() {
    let e = &dict().lookup_at(LINE, 0)[0].entries[0];
    let ruby: Vec<(&str, Option<&str>)> = e
        .furigana
        .iter()
        .map(|f| (f.text.as_str(), f.ruby.as_deref()))
        .collect();
    assert_eq!(ruby, [("夕焼", Some("ゆうや")), ("け", None)]);
    assert_eq!(e.pitch.len(), 1);
    assert_eq!((e.pitch[0].downstep, e.pitch[0].pattern), (0, "heiban"));
    assert_eq!(e.pitch[0].morae, ["ゆ", "う", "や", "け"]);
    assert_eq!(e.senses[0].pos, ["noun"]);
    assert_eq!(e.senses[0].glosses, ["evening glow", "red sunset"]);
}

#[test]
fn search_only_form_is_matched_but_headword_stays_normal() {
    let r = dict().lookup_at("夕焼", 0);
    assert_eq!(r[0].matched, "夕焼");
    assert_eq!(r[0].entries[0].term, "夕焼け");
    let r = dict().lookup_at("喰べた", 0);
    assert_eq!(r[0].entries[0].term, "食べる");
    assert_eq!(r[0].inflections, ["past"]);
}

#[test]
fn other_words_of_the_sample_line() {
    let r = lookup(LINE, "綺麗");
    assert_eq!(r[0].matched, "綺麗"); // not 綺麗だ: だ is its own word
    let e = &r[0].entries[0];
    assert_eq!((e.term.as_str(), e.reading.as_str()), ("綺麗", "きれい"));
    assert_eq!(e.other_forms, ["奇麗"]);
    assert_eq!(e.senses.len(), 2);
    assert_eq!(e.pitch[0].pattern, "atamadaka");
    assert_eq!(lookup(LINE, "こんなに")[0].matched, "こんな");
    assert_eq!(lookup(LINE, "なんて")[0].matched, "なんて");
    assert_eq!(lookup(LINE, "だなんて")[0].matched, "だ");
}

#[test]
fn conjugated_words_reduce_to_dictionary_form() {
    let r = lookup(LINE, "知らなかった");
    assert_eq!(r[0].matched, "知らなかった");
    assert_eq!(r[0].deinflected.as_deref(), Some("知る"));
    assert_eq!(r[0].inflections, ["negative", "past"]);
    assert_eq!(terms(&r[0]), ["知る"]);
    assert_eq!(r[0].entries[0].pitch[0].pattern, "heiban");

    let r = dict().lookup_at("諦めない", 0);
    assert_eq!(r[0].matched, "諦めない");
    assert_eq!(
        (r[0].deinflected.as_deref(), r[0].inflections.as_slice()),
        (Some("諦める"), ["negative".to_string()].as_slice())
    );
    let e = &r[0].entries[0];
    assert_eq!(
        e.furigana
            .iter()
            .map(|f| (f.text.as_str(), f.ruby.as_deref()))
            .collect::<Vec<_>>(),
        [("諦", Some("あきら")), ("める", None)]
    );
    assert_eq!((e.pitch[0].downstep, e.pitch[0].pattern), (4, "nakadaka"));

    let r = dict().lookup_at("食べさせられなかった", 0);
    assert_eq!(
        r[0].inflections,
        ["causative", "passive", "negative", "past"]
    );
    assert_eq!(terms(&r[0]), ["食べる"]);
    assert_eq!(r[0].len, 10);

    for (text, term, chain) in [
        ("決めた", "決める", vec!["past"]),
        ("行こう", "行く", vec!["volitional"]),
        ("行った", "行く", vec!["past"]),
        ("持ってきた", "持つ", vec!["te-kuru", "past"]),
        ("撮ろう", "撮る", vec!["volitional"]),
        ("忘れないで", "忘れる", vec!["negative", "te-form"]),
        ("会おう", "会う", vec!["volitional"]),
        ("書いている", "書く", vec!["progressive"]),
        ("来なかった", "来る", vec!["negative", "past"]),
        ("高くなかった", "高い", vec!["negative", "past"]),
        ("よかった", "良い", vec!["past"]),
    ] {
        let r = dict().lookup_at(text, 0);
        assert_eq!(r[0].matched, text, "{text}");
        assert_eq!(terms(&r[0])[0], term, "{text}");
        assert_eq!(r[0].inflections, chain, "{text}");
    }
}

#[test]
fn conjugation_must_agree_with_the_entry_part_of_speech() {
    // 書く is godan-ku: 書いた is fine, but an ichidan reading of "...た" must not pull in other entries.
    assert_eq!(terms(&dict().lookup_at("書いた", 0)[0]), ["書く"]);
    // 見た -> 見る (ichidan), not a godan verb.
    assert_eq!(terms(&dict().lookup_at("見た", 0)[0]), ["見る"]);
    // "食べな" is nothing; only the masu stem 食べ inside it is a word.
    let r = dict().lookup_at("食べな", 0);
    assert_eq!(r.len(), 1);
    assert_eq!(
        (r[0].matched.as_str(), r[0].inflections.as_slice()),
        ("食べ", ["masu stem".to_string()].as_slice())
    );
}

#[test]
fn suru_nouns_and_katakana() {
    let r = dict().lookup_at("勉強した", 0);
    assert_eq!(r[0].matched, "勉強した");
    assert_eq!(r[0].deinflected.as_deref(), Some("勉強する"));
    assert_eq!(r[0].inflections, ["past"]);
    assert_eq!(terms(&r[0]), ["勉強"]);

    let r = dict().lookup_at("ケーキを食べた", 0);
    assert_eq!(r[0].matched, "ケーキ");
    assert_eq!(r[0].entries[0].pitch[0].pattern, "atamadaka");
    // Hiragana spelling of a katakana word finds it too, and a verb that is usually kana shows the kana.
    assert_eq!(dict().lookup_at("けーき", 0)[0].entries[0].term, "ケーキ");
    let r = dict().lookup_at("した", 0);
    assert_eq!(
        (r[0].deinflected.as_deref(), r[0].entries[0].term.as_str()),
        (Some("する"), "する")
    );
    assert_eq!(
        dict().lookup_at("ありがとう", 0)[0].entries[0].term,
        "ありがとう"
    );
}

#[test]
fn clicks_always_start_at_the_clicked_character() {
    // The UI passes the start of the word unit under the cursor. A click on め of 諦めない starts the scan at め:
    // it never looks backwards, so 諦める is not found from there.
    let text = "諦めない";
    let r = dict().lookup_at(text, offset_of(text, "め"));
    assert!(
        r.iter()
            .all(|r| r.entries.iter().all(|e| e.term != "諦める")),
        "{r:?}"
    );
}

#[test]
fn odd_input_does_not_crash() {
    let d = dict();
    assert!(d.lookup_at("", 0).is_empty());
    assert!(d.lookup_at("夕焼け", 3).is_empty());
    assert!(d.lookup_at("夕焼け", 999).is_empty());
    assert!(d.lookup_at("「夕焼け」", 0).is_empty());
    assert!(d.lookup_at("夕焼け、綺麗", 3).is_empty());
    assert!(
        d.lookup_at("すっげー", 0)
            .iter()
            .all(|r| !r.matched.is_empty())
    );
    let _ = d.lookup_at("😀😀😀ー〜…", 1);
    let _ = d.lookup_at(&"ああ".repeat(200), 5);
    let _ = d.lookup_at("a'b\"c%_;--", 0);
    // A match never crosses punctuation.
    assert_eq!(d.lookup_at("夕焼け、綺麗", 0)[0].matched, "夕焼け");
}

#[test]
fn meta_and_attribution_are_recorded() {
    let d = dict();
    assert_eq!(d.meta("jmdict_version").as_deref(), Some("3.6.2"));
    assert_eq!(
        d.meta("kanjium_commit").as_deref(),
        Some(jpdict::build::KANJIUM_COMMIT)
    );
    assert!(
        d.meta("attribution")
            .unwrap()
            .contains("Electronic Dictionary Research and Development Group")
    );
    assert_eq!(d.meta("entry_count").as_deref(), Some("26"));
}

#[test]
fn building_from_a_missing_file_fails_cleanly() {
    let dir = common::temp_dir("missing");
    let out = dir.join("x.sqlite");
    assert!(
        jpdict::build::build_dict(&dir.join("nope.json"), &dir.join("nope.txt"), &out).is_err()
    );
    assert!(!out.exists() && !dir.join("x.sqlite.tmp").exists());
    // Garbage input is an error, not a panic.
    std::fs::write(
        dir.join("bad.json"),
        "{\"tags\": {\"a\": \"b\"}, \"words\": [1,2]}",
    )
    .unwrap();
    std::fs::write(dir.join("a.txt"), "").unwrap();
    assert!(jpdict::build::build_dict(&dir.join("bad.json"), &dir.join("a.txt"), &out).is_err());
}

// ---- JSON shape against the TypeScript contract --------------------------------------------------

/// Field names of `interface <name> { ... }` in backend.ts: (name, optional).
fn ts_fields(src: &str, name: &str) -> Vec<(String, bool)> {
    let start = src
        .find(&format!("export interface {name} {{"))
        .unwrap_or_else(|| panic!("interface {name} not found"));
    let body = &src[start..];
    let end = body.find("\n}\n").unwrap();
    body[..end]
        .lines()
        .skip(1)
        .filter(|l| {
            l.starts_with("  ")
                && !l.starts_with("   ")
                && !l.trim_start().starts_with("/")
                && !l.trim_start().starts_with('*')
        })
        .filter_map(|l| {
            let (n, _) = l.trim().split_once(':')?;
            Some((n.trim_end_matches('?').to_string(), n.ends_with('?')))
        })
        .collect()
}

fn check_shape(value: &Value, src: &str, iface: &str) {
    let fields = ts_fields(src, iface);
    assert!(!fields.is_empty(), "{iface}");
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{iface}: not an object"));
    for (name, optional) in &fields {
        assert!(
            *optional || obj.contains_key(name),
            "{iface}.{name} missing in {value}"
        );
    }
    for key in obj.keys() {
        assert!(
            fields.iter().any(|(n, _)| n == key),
            "{iface}: unexpected key {key}"
        );
    }
}

#[test]
fn json_matches_the_typescript_contract() {
    let ts = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/lib/backend.ts"
    ))
    .unwrap();
    let results = dict().lookup_at(LINE, 0);
    let plain = serde_json::to_value(&results).unwrap();
    let inflected = serde_json::to_value(dict().lookup_at("諦めない", 0)).unwrap();
    let mut saw_deinflected = false;
    for v in plain
        .as_array()
        .unwrap()
        .iter()
        .chain(inflected.as_array().unwrap())
    {
        check_shape(v, &ts, "LookupResult");
        saw_deinflected |= v.get("deinflected").is_some();
        assert!(
            v["start"].is_u64()
                && v["len"].is_u64()
                && v["matched"].is_string()
                && v["inflections"].is_array()
        );
        for e in v["entries"].as_array().unwrap() {
            check_shape(e, &ts, "DictEntry");
            assert!(e["id"].is_i64() && e["common"].is_boolean() && e["otherForms"].is_array());
            for s in e["senses"].as_array().unwrap() {
                check_shape(s, &ts, "Sense");
            }
            for p in e["pitch"].as_array().unwrap() {
                check_shape(p, &ts, "Pitch");
                assert!(
                    ["heiban", "atamadaka", "nakadaka", "odaka"]
                        .contains(&p["pattern"].as_str().unwrap())
                );
            }
            for f in e["furigana"].as_array().unwrap() {
                let keys: Vec<&String> = f.as_object().unwrap().keys().collect();
                assert!(keys.iter().all(|k| *k == "text" || *k == "ruby") && f["text"].is_string());
            }
            assert!(
                e.get("freqRank").is_none(),
                "optional field must be omitted when unknown"
            );
        }
    }
    assert!(saw_deinflected);
    // Optional fields are omitted, not null.
    assert!(plain[0].get("deinflected").is_none());
}
