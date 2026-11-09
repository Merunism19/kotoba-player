//! A tiny JMdict fixture in the jmdict-simplified format and a Kanjium accents fixture; builds a real database from them.
#![allow(dead_code)]

use std::path::PathBuf;

use jpdict::Dict;
use serde_json::{Value, json};

/// (id, kanji [(text, common, tags)], kana [(text, common, appliesToKanji)], senses [(pos tags, misc tags, glosses)])
type Spec<'a> = (
    u32,
    Vec<(&'a str, bool, Vec<&'a str>)>,
    Vec<(&'a str, bool, Vec<&'a str>)>,
    Vec<(Vec<&'a str>, Vec<&'a str>, Vec<&'a str>)>,
);

fn word(spec: Spec) -> Value {
    let (id, kanji, kana, senses) = spec;
    json!({
        "id": id.to_string(),
        "kanji": kanji.iter().map(|(t, c, g)| json!({"common": c, "text": t, "tags": g})).collect::<Vec<_>>(),
        "kana": kana.iter().map(|(t, c, a)| json!({"common": c, "text": t, "tags": [], "appliesToKanji": if a.is_empty() { vec!["*"] } else { a.clone() }})).collect::<Vec<_>>(),
        "sense": senses.iter().map(|(pos, misc, glosses)| json!({
            "partOfSpeech": pos, "appliesToKanji": ["*"], "appliesToKana": ["*"], "related": [], "antonym": [],
            "field": [], "dialect": [], "misc": misc, "info": [], "languageSource": [],
            "gloss": glosses.iter().map(|g| json!({"lang": "eng", "gender": null, "type": null, "text": g})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

pub fn fixture_json() -> String {
    let tags = json!({
        "v1": "Ichidan verb", "v5r": "Godan verb with 'ru' ending", "v5k": "Godan verb with 'ku' ending",
        "v5k-s": "Godan verb - Iku/Yuku special class", "v5t": "Godan verb with 'tsu' ending", "v5u": "Godan verb with 'u' ending",
        "vk": "Kuru verb - special class", "vs-i": "suru verb - included", "vs": "noun or participle which takes the aux. verb suru",
        "vt": "transitive verb", "vi": "intransitive verb", "n": "noun (common) (futsuumeishi)",
        "adj-na": "adjectival nouns or quasi-adjectives (keiyodoshi)", "adj-i": "adjective (keiyoushi)",
        "adj-ix": "adjective (keiyoushi) - yoi/ii class", "adj-pn": "pre-noun adjectival (rentaishi)", "prt": "particle",
        "exp": "expressions (phrases, clauses, etc.)", "cop": "copula", "uk": "word usually written using kana alone",
        "sK": "search-only kanji form", "int": "interjection (kandoushi)",
    });
    let n = vec!["n"];
    let words: Vec<Value> = vec![
        word((
            1542720,
            vec![("夕焼け", true, vec![]), ("夕焼", false, vec!["sK"])],
            vec![("ゆうやけ", true, vec![])],
            vec![(n.clone(), vec![], vec!["evening glow", "red sunset"])],
        )),
        word((
            1000001,
            vec![("夕", true, vec![])],
            vec![("ゆう", true, vec![])],
            vec![(n.clone(), vec![], vec!["evening"])],
        )),
        word((
            1436730,
            vec![("諦める", true, vec![])],
            vec![("あきらめる", true, vec![])],
            vec![(
                vec!["v1", "vt"],
                vec![],
                vec!["to give up", "to abandon (hope, plans)"],
            )],
        )),
        word((
            1000002,
            vec![("決める", true, vec![])],
            vec![("きめる", true, vec![])],
            vec![(vec!["v1", "vt"], vec![], vec!["to decide"])],
        )),
        word((
            1358280,
            vec![("食べる", true, vec![]), ("喰べる", false, vec!["sK"])],
            vec![("たべる", true, vec![])],
            vec![(vec!["v1", "vt"], vec![], vec!["to eat"])],
        )),
        word((
            1000003,
            vec![("知る", true, vec![])],
            vec![("しる", true, vec![])],
            vec![(vec!["v5r", "vt"], vec![], vec!["to know", "to be aware of"])],
        )),
        word((
            1000004,
            vec![("行く", true, vec![])],
            vec![("いく", true, vec![]), ("ゆく", false, vec![])],
            vec![(vec!["v5k-s", "vi"], vec![], vec!["to go"])],
        )),
        word((
            1000005,
            vec![("持つ", true, vec![])],
            vec![("もつ", true, vec![])],
            vec![(vec!["v5t", "vt"], vec![], vec!["to hold", "to carry"])],
        )),
        word((
            1000006,
            vec![("撮る", true, vec![])],
            vec![("とる", true, vec![])],
            vec![(vec!["v5r", "vt"], vec![], vec!["to take (a photo)"])],
        )),
        word((
            1000007,
            vec![("忘れる", true, vec![])],
            vec![("わすれる", true, vec![])],
            vec![(vec!["v1", "vt"], vec![], vec!["to forget"])],
        )),
        word((
            1000008,
            vec![("会う", true, vec![])],
            vec![("あう", true, vec![])],
            vec![(vec!["v5u", "vi"], vec![], vec!["to meet"])],
        )),
        word((
            1000009,
            vec![("綺麗", true, vec![]), ("奇麗", false, vec![])],
            vec![("きれい", true, vec![])],
            vec![
                (vec!["adj-na"], vec![], vec!["pretty", "beautiful"]),
                (vec!["adj-na"], vec![], vec!["clean"]),
            ],
        )),
        word((
            1000010,
            vec![("来る", true, vec![])],
            vec![("くる", true, vec![])],
            vec![(vec!["vk", "vi"], vec![], vec!["to come"])],
        )),
        word((
            1157170,
            vec![("為る", false, vec![])],
            vec![("する", true, vec![])],
            vec![(vec!["vs-i", "vt"], vec!["uk"], vec!["to do"])],
        )),
        word((
            1000011,
            vec![("勉強", true, vec![])],
            vec![("べんきょう", true, vec![])],
            vec![(vec!["n", "vs", "vt"], vec![], vec!["study"])],
        )),
        word((
            1000012,
            vec![("高い", true, vec![])],
            vec![("たかい", true, vec![])],
            vec![
                (vec!["adj-i"], vec![], vec!["high", "tall"]),
                (vec!["adj-i"], vec![], vec!["expensive"]),
            ],
        )),
        word((
            1000013,
            vec![],
            vec![("ケーキ", true, vec![])],
            vec![(n.clone(), vec![], vec!["cake"])],
        )),
        word((
            1000014,
            vec![],
            vec![("が", true, vec![])],
            vec![(vec!["prt"], vec![], vec!["subject marker"])],
        )),
        word((
            1000015,
            vec![],
            vec![("こんな", true, vec![])],
            vec![(vec!["adj-pn"], vec![], vec!["such"])],
        )),
        word((
            1000016,
            vec![],
            vec![("なんて", true, vec![])],
            vec![(vec!["prt"], vec![], vec!["such a thing as"])],
        )),
        word((
            1000017,
            vec![],
            vec![("だ", true, vec![])],
            vec![(vec!["cop"], vec![], vec!["be"])],
        )),
        word((
            1000018,
            vec![("書く", true, vec![])],
            vec![("かく", true, vec![])],
            vec![(vec!["v5k", "vt"], vec![], vec!["to write"])],
        )),
        word((
            1000019,
            vec![("見る", true, vec![])],
            vec![("みる", true, vec![])],
            vec![(vec!["v1", "vt"], vec![], vec!["to see"])],
        )),
        word((
            1000020,
            vec![("良い", true, vec![])],
            vec![("よい", true, vec![]), ("いい", true, vec![])],
            vec![(vec!["adj-ix"], vec![], vec!["good"])],
        )),
        word((
            1000021,
            vec![("有難う", true, vec![])],
            vec![("ありがとう", true, vec![])],
            vec![(vec!["exp", "int"], vec!["uk"], vec!["thank you"])],
        )),
        word((
            1000022,
            vec![("に", false, vec![])],
            vec![("に", true, vec![])],
            vec![(vec!["prt"], vec![], vec!["indicates direction"])],
        )),
    ];
    serde_json::to_string(&json!({
        "version": "3.6.2", "languages": ["eng"], "commonOnly": false, "dictDate": "2026-10-26",
        "dictRevisions": ["1.09"], "tags": tags, "words": words,
    }))
    .unwrap()
}

pub const ACCENTS: &str = "諦める\tあきらめる\t4\n夕焼け\tゆうやけ\t0\n食べる\tたべる\t2\n綺麗\tきれい\t1\n知る\tしる\t0\nケーキ\tケーキ\t1\n来る\tくる\t1\n高い\tたかい\t2\nが\tが\t0,1\n";

pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jpdict-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build the fixture database and open it.
pub fn fixture_dict(name: &str) -> Dict {
    let dir = temp_dir(name);
    let json = dir.join("jmdict.json");
    let accents = dir.join("accents.txt");
    let db = dir.join("dict.sqlite");
    std::fs::write(&json, fixture_json()).unwrap();
    std::fs::write(&accents, ACCENTS).unwrap();
    jpdict::build::build_dict(&json, &accents, &db).unwrap();
    Dict::open(&db).unwrap()
}

/// Code-point offset of the first occurrence of `needle` in `hay`.
pub fn offset_of(hay: &str, needle: &str) -> usize {
    let byte = hay.find(needle).unwrap();
    hay[..byte].chars().count()
}
