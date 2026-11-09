//! Build the SQLite dictionary from the JMdict JSON (jmdict-simplified format) and Kanjium's `accents.txt`.
//!
//! Schema (version [`SCHEMA_VERSION`]):
//!
//! * `meta(key, value)`: source versions and attribution.
//! * `entries(id, data)`: one JSON record per JMdict entry (see `store.rs`).
//! * `forms(text, entry_id, kind, idx, pri)`: every written form (kind 0) and reading (kind 1) pointing at its
//!   entry, with a ranking hint `pri` (form rarity * 2 + 1 if the entry is not common) so lookups can rank without loading entries; katakana readings are also stored hiragana-normalized. `WITHOUT ROWID`, so the primary key is the lookup index.
//! * `pitch(term, reading, down)`: Kanjium accents, merged per (term, reading); `down` is "0,2"-style.
//!
//! The JSON is parsed as a stream: `words` is visited element by element and written straight to SQLite.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::{Connection, params};
use serde::Deserialize;
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::error::Error;
use crate::kana::to_hiragana;
use crate::pitch::parse_downsteps;
use crate::store::{Form, SCHEMA_VERSION, SenseRec, Stored};

/// The Kanjium commit `accents.txt` is pinned to (recorded in `meta`).
pub const KANJIUM_COMMIT: &str = "9ebca4589565c696e7c39e089e2e928ad65f678c";

/// What a finished build produced.
#[derive(Debug, Clone)]
pub struct BuildStats {
    pub entries: u64,
    pub forms: u64,
    pub pitch_rows: u64,
    pub db_bytes: u64,
    pub seconds: f64,
}

// ---- input records (jmdict-simplified) --------------------------------------------------------

#[derive(Deserialize)]
#[serde(untagged)]
enum IdRepr {
    S(String),
    N(i64),
}

#[derive(Deserialize)]
struct WKanji {
    #[serde(default)]
    common: bool,
    text: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct WKana {
    #[serde(default)]
    common: bool,
    text: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "appliesToKanji")]
    applies_to_kanji: Vec<String>,
}

#[derive(Deserialize)]
struct WGloss {
    text: String,
}

#[derive(Deserialize)]
struct WSense {
    #[serde(default, rename = "partOfSpeech")]
    pos: Vec<String>,
    #[serde(default, rename = "appliesToKanji")]
    applies_to_kanji: Vec<String>,
    #[serde(default, rename = "appliesToKana")]
    applies_to_kana: Vec<String>,
    #[serde(default)]
    field: Vec<String>,
    #[serde(default)]
    dialect: Vec<String>,
    #[serde(default)]
    misc: Vec<String>,
    #[serde(default)]
    info: Vec<String>,
    #[serde(default)]
    gloss: Vec<WGloss>,
}

#[derive(Deserialize)]
struct Word {
    id: IdRepr,
    #[serde(default)]
    kanji: Vec<WKanji>,
    #[serde(default)]
    kana: Vec<WKana>,
    #[serde(default)]
    sense: Vec<WSense>,
}

// ---- tag labels --------------------------------------------------------------------------------

/// Romaji glosses JMdict appends to some tag descriptions ("adjective (keiyoushi)").
const ROMAJI_SUFFIXES: &[&str] = &[
    "keiyoushi",
    "futsuumeishi",
    "jisoumeishi",
    "fukushitekimeishi",
    "fukushi",
    "keiyodoshi",
    "kandoushi",
    "rentaishi",
];

/// Plain-English label for a JMdict tag, from the file's own tag dictionary.
fn label(tag: &str, tags: &HashMap<String, String>) -> String {
    match tag {
        "n" => return "noun".into(),
        "adj-i" => return "i-adjective".into(),
        "adj-na" => return "na-adjective".into(),
        "vt" => return "transitive".into(),
        "vi" => return "intransitive".into(),
        _ => {}
    }
    let Some(desc) = tags.get(tag) else {
        return tag.to_string();
    };
    let mut s = desc.trim().to_string();
    loop {
        let stripped = s.strip_suffix(')').and_then(|r| {
            r.rfind(" (")
                .map(|i| (r[i + 2..].to_string(), r[..i].to_string()))
        });
        match stripped {
            Some((inner, rest)) if ROMAJI_SUFFIXES.contains(&inner.as_str()) => s = rest,
            _ => break,
        }
    }
    s
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

/// Indices of the forms named in `names` (an applies-to list); empty when it says "*" or names nothing known.
fn restriction(names: &[String], forms: &[Form]) -> Vec<u16> {
    if names.iter().any(|x| x == "*") {
        return Vec::new();
    }
    names
        .iter()
        .filter_map(|n| forms.iter().position(|f| &f.t == n))
        .map(|i| i as u16)
        .collect()
}

fn to_stored(w: Word) -> (i64, Stored) {
    let id = match w.id {
        IdRepr::N(n) => n,
        IdRepr::S(s) => s.parse().unwrap_or(0),
    };
    let k: Vec<Form> = w
        .kanji
        .into_iter()
        .map(|k| Form {
            t: k.text,
            c: u8::from(k.common),
            g: k.tags,
            a: Vec::new(),
        })
        .collect();
    let r: Vec<Form> = w
        .kana
        .into_iter()
        .map(|r| Form {
            a: restriction(&r.applies_to_kanji, &k),
            t: r.text,
            c: u8::from(r.common),
            g: r.tags,
        })
        .collect();
    let mut s = Vec::new();
    let mut prev_pos: Vec<String> = Vec::new();
    for sense in w.sense {
        let mut notes: Vec<String> = Vec::new();
        for t in sense.misc.iter().chain(&sense.field).chain(&sense.dialect) {
            push_unique(&mut notes, t.clone());
        }
        // Store the part-of-speech codes only when they differ from the previous sense's.
        let p = if sense.pos == prev_pos {
            Vec::new()
        } else {
            sense.pos.clone()
        };
        prev_pos = sense.pos.clone();
        s.push(SenseRec {
            g: sense.gloss.into_iter().map(|g| g.text).collect(),
            p,
            n: notes,
            i: sense.info,
            u: sense.misc.iter().any(|m| m == "uk"),
            ak: restriction(&sense.applies_to_kanji, &k),
            ar: restriction(&sense.applies_to_kana, &r),
        });
    }
    (id, Stored { k, r, s })
}

// ---- streaming parse ------------------------------------------------------------------------------

#[derive(Default)]
struct SourceMeta {
    version: String,
    dict_date: String,
    revisions: Vec<String>,
    tags: HashMap<String, String>,
}

type OnWord<'a> = &'a mut dyn FnMut(Word, &HashMap<String, String>) -> Result<(), String>;

struct RootVisitor<'a> {
    meta: &'a mut SourceMeta,
    on_word: OnWord<'a>,
}

impl<'de, 'a> Visitor<'de> for RootVisitor<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a jmdict-simplified JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut tags: HashMap<String, String> = HashMap::new();
        let mut saw_words = false;
        let on_word = self.on_word;
        let mut on_word = Some(on_word);
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "version" => self.meta.version = map.next_value()?,
                "dictDate" => self.meta.dict_date = map.next_value()?,
                "dictRevisions" => self.meta.revisions = map.next_value()?,
                "tags" => {
                    tags = map.next_value()?;
                    self.meta.tags = tags.clone();
                }
                "words" => {
                    if tags.is_empty() {
                        return Err(de::Error::custom(
                            "\"tags\" must come before \"words\" in the file",
                        ));
                    }
                    let f = on_word
                        .take()
                        .ok_or_else(|| de::Error::custom("duplicate \"words\""))?;
                    map.next_value_seed(WordsSeed {
                        tags: &tags,
                        on_word: f,
                    })?;
                    saw_words = true;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        if !saw_words {
            return Err(de::Error::custom("no \"words\" array"));
        }
        Ok(())
    }
}

struct WordsSeed<'a> {
    tags: &'a HashMap<String, String>,
    on_word: OnWord<'a>,
}

impl<'de, 'a> DeserializeSeed<'de> for WordsSeed<'a> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de, 'a> Visitor<'de> for WordsSeed<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an array of words")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(w) = seq.next_element::<Word>()? {
            (self.on_word)(w, self.tags).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

// ---- the build ----------------------------------------------------------------------------------------

// Licence attribution text required by EDRDG and Kanjium (stored in the DB for the About screen; never fetched).
// network-guard: ok attribution URL, never fetched
const EDRDG_LICENCE_URL: &str = "https://www.edrdg.org/edrdg/licence.html";
// network-guard: ok attribution URL, never fetched
const KANJIUM_URL: &str = "https://github.com/mifunetoshiro/kanjium";
/// The attribution sentence (with the licence and source links) stored in the DB's `meta` table.
fn attribution() -> String {
    format!(
        "This dictionary uses JMdict/EDICT data. It is the property of the Electronic Dictionary Research and \
         Development Group (EDRDG), and is used in conformance with the Group's licence (CC BY-SA 4.0): \
         {EDRDG_LICENCE_URL}. Pitch accents are from Kanjium (CC BY-SA 4.0): {KANJIUM_URL}."
    )
}

fn tmp_path(out: &Path) -> PathBuf {
    let mut name = out
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    out.with_file_name(name)
}

/// Build `out` from the JMdict JSON at `jmdict` and Kanjium's `accents` file.
/// The database is written to `<out>.tmp` and renamed on success.
pub fn build_dict(jmdict: &Path, accents: &Path, out: &Path) -> Result<BuildStats, Error> {
    let started = Instant::now();
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = tmp_path(out);
    let _ = std::fs::remove_file(&tmp);
    let result = build_into(jmdict, accents, &tmp);
    match result {
        Ok(mut stats) => {
            let _ = std::fs::remove_file(out);
            std::fs::rename(&tmp, out)?;
            stats.db_bytes = std::fs::metadata(out)?.len();
            stats.seconds = started.elapsed().as_secs_f64();
            Ok(stats)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn build_into(jmdict: &Path, accents: &Path, db_path: &Path) -> Result<BuildStats, Error> {
    let mut conn = Connection::open(db_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;
         CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
         CREATE TABLE entries(id INTEGER PRIMARY KEY, data TEXT NOT NULL);
         CREATE TABLE forms(text TEXT NOT NULL, entry_id INTEGER NOT NULL, kind INTEGER NOT NULL, idx INTEGER NOT NULL, pri INTEGER NOT NULL,
                            PRIMARY KEY(text, entry_id, kind, idx)) WITHOUT ROWID;
         CREATE TABLE tags(code TEXT PRIMARY KEY, label TEXT NOT NULL) WITHOUT ROWID;
         CREATE TABLE pitch(term TEXT NOT NULL, reading TEXT NOT NULL, down TEXT NOT NULL, PRIMARY KEY(term, reading)) WITHOUT ROWID;
         CREATE INDEX pitch_reading ON pitch(reading);",
    )?;

    let tx = conn.transaction()?;
    let mut entries: u64 = 0;
    let mut forms: u64 = 0;
    let mut src = SourceMeta::default();
    {
        let mut ins_entry =
            tx.prepare("INSERT OR REPLACE INTO entries(id, data) VALUES (?1, ?2)")?;
        let mut ins_form = tx.prepare("INSERT OR IGNORE INTO forms(text, entry_id, kind, idx, pri) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        let mut on_word = |w: Word, _tags: &HashMap<String, String>| -> Result<(), String> {
            let (id, stored) = to_stored(w);
            let json = serde_json::to_string(&stored).map_err(|e| e.to_string())?;
            ins_entry
                .execute(params![id, json])
                .map_err(|e| e.to_string())?;
            let not_common = i64::from(!stored.k.iter().chain(stored.r.iter()).any(Form::common));
            let pri = |f: &Form| -> i64 {
                let penalty = if f.search_only() {
                    2
                } else if f.rare() {
                    1
                } else {
                    0
                };
                penalty * 2 + not_common
            };
            for (i, k) in stored.k.iter().enumerate() {
                ins_form
                    .execute(params![k.t, id, 0, i as i64, pri(k)])
                    .map_err(|e| e.to_string())?;
                forms += 1;
            }
            for (i, r) in stored.r.iter().enumerate() {
                ins_form
                    .execute(params![r.t, id, 1, i as i64, pri(r)])
                    .map_err(|e| e.to_string())?;
                forms += 1;
                let norm = to_hiragana(&r.t);
                if norm != r.t {
                    ins_form
                        .execute(params![norm, id, 1, i as i64, pri(r)])
                        .map_err(|e| e.to_string())?;
                    forms += 1;
                }
            }
            entries += 1;
            Ok(())
        };
        let file = File::open(jmdict)?;
        let reader = BufReader::with_capacity(1 << 20, file);
        let mut de = serde_json::Deserializer::from_reader(reader);
        de::Deserializer::deserialize_map(
            &mut de,
            RootVisitor {
                meta: &mut src,
                on_word: &mut on_word,
            },
        )?;
    }

    // Kanjium accents: term <TAB> reading <TAB> downsteps; merge duplicates per (term, reading).
    let mut merged: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    let reader = BufReader::new(File::open(accents)?);
    for line in reader.lines() {
        let line = line?;
        let mut cols = line.trim_end_matches('\r').split('\t');
        let (Some(term), Some(reading), Some(down)) = (cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        if term.is_empty() {
            continue;
        }
        // Kana-only terms have an empty reading column: the term is the reading.
        let reading = if reading.is_empty() { term } else { reading };
        let v = merged
            .entry((term.to_string(), to_hiragana(reading)))
            .or_default();
        for d in parse_downsteps(down) {
            if !v.contains(&d) {
                v.push(d);
            }
        }
    }
    let mut pitch_rows: u64 = 0;
    {
        let mut ins = tx.prepare("INSERT INTO pitch(term, reading, down) VALUES (?1, ?2, ?3)")?;
        for ((term, reading), downs) in &merged {
            if downs.is_empty() {
                continue;
            }
            let text = downs
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            ins.execute(params![term, reading, text])?;
            pitch_rows += 1;
        }
    }

    if entries == 0 {
        return Err(Error::Format("the JMdict file has no words".into()));
    }
    {
        let mut ins = tx.prepare("INSERT INTO tags(code, label) VALUES (?1, ?2)")?;
        for code in src.tags.keys() {
            ins.execute(params![code, label(code, &src.tags)])?;
        }
    }
    {
        let mut ins = tx.prepare("INSERT INTO meta(key, value) VALUES (?1, ?2)")?;
        let built_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let rows: [(&str, String); 10] = [
            ("schema_version", SCHEMA_VERSION.to_string()),
            ("jmdict_version", src.version),
            ("jmdict_date", src.dict_date),
            ("jmdict_revisions", src.revisions.join(",")),
            // network-guard: ok source credit stored in the DB; never fetched
            ("jmdict_source", "JMdict English via jmdict-simplified (https://github.com/scriptin/jmdict-simplified)".to_string()),
            ("kanjium_commit", KANJIUM_COMMIT.to_string()),
            ("attribution", attribution()),
            ("entry_count", entries.to_string()),
            ("pitch_count", pitch_rows.to_string()),
            ("built_at_unix", built_at.to_string()),
        ];
        for (k, v) in rows {
            ins.execute(params![k, v])?;
        }
    }
    tx.commit()?;
    conn.execute_batch("VACUUM;")?;
    drop(conn);

    Ok(BuildStats {
        entries,
        forms,
        pitch_rows,
        db_bytes: 0,
        seconds: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_plain_english() {
        let mut tags = HashMap::new();
        tags.insert("v1".to_string(), "Ichidan verb".to_string());
        tags.insert("n".to_string(), "noun (common) (futsuumeishi)".to_string());
        tags.insert("adv".to_string(), "adverb (fukushi)".to_string());
        tags.insert("v5k".to_string(), "Godan verb with 'ku' ending".to_string());
        tags.insert(
            "uk".to_string(),
            "word usually written using kana alone".to_string(),
        );
        tags.insert(
            "n-t".to_string(),
            "noun (temporal) (jisoumeishi)".to_string(),
        );
        assert_eq!(label("v1", &tags), "Ichidan verb");
        assert_eq!(label("n", &tags), "noun");
        assert_eq!(label("adj-i", &tags), "i-adjective");
        assert_eq!(label("vt", &tags), "transitive");
        assert_eq!(label("adv", &tags), "adverb");
        assert_eq!(label("n-t", &tags), "noun (temporal)");
        assert_eq!(label("v5k", &tags), "Godan verb with 'ku' ending");
        assert_eq!(label("uk", &tags), "word usually written using kana alone");
        assert_eq!(label("zzz", &tags), "zzz");
    }
}
