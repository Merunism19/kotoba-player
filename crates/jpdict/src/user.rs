//! User dictionaries: importing Yomitan-format zips into a separate SQLite file.
//!
//! The file holds `dicts` (one row per import), `uterms` (term entries), `ufreq` (frequency ranks) and
//! `upitch` (pitch accents). [`crate::Dict`] attaches it as schema `u` next to the read-only JMdict file.
//!
//! The zip is only ever read into memory: no entry is written to disk, so a hostile entry name cannot
//! escape anywhere; names that try anyway (`..`, absolute paths, drive letters, NUL, backslashes) reject
//! the whole archive. Sizes, entry counts, row counts, text lengths and JSON depth are capped.

use std::io::{BufReader, Read};
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::de::{self, Deserializer as _, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::deinflect::{self, V5ALL, VSN};
use crate::error::Error;
use crate::kana::to_hiragana;

/// Largest total uncompressed size of one archive.
pub const MAX_TOTAL_BYTES: u64 = 1 << 30;
/// Largest uncompressed size of one archive member we read.
pub const MAX_FILE_BYTES: u64 = 300 << 20;
/// Most members in one archive.
pub const MAX_ENTRIES: usize = 10_000;
/// Most term / frequency / pitch rows in one archive.
pub const MAX_ROWS: usize = 10_000_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_TERM_CHARS: usize = 200;
const MAX_GLOSS_CHARS: usize = 400;
const MAX_GLOSSES: usize = 40;
const MAX_ENTRY_CHARS: usize = 6000;
const MAX_FLATTEN_DEPTH: usize = 40;

const USER_VERSION: i64 = 1;

/// What an imported dictionary contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportKind {
    /// Term entries (definitions), possibly with pitch accents.
    Terms,
    /// Frequency ranks only.
    Frequency,
    /// Terms and frequencies.
    Both,
    /// Pitch accents only.
    Pitch,
}

impl ImportKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ImportKind::Terms => "terms",
            ImportKind::Frequency => "frequency",
            ImportKind::Both => "both",
            ImportKind::Pitch => "pitch",
        }
    }
    pub(crate) fn parse(s: &str) -> ImportKind {
        match s {
            "frequency" => ImportKind::Frequency,
            "both" => ImportKind::Both,
            "pitch" => ImportKind::Pitch,
            _ => ImportKind::Terms,
        }
    }
}

/// The result of a successful import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub title: String,
    pub revision: String,
    pub kind: ImportKind,
    pub terms: usize,
    pub frequencies: usize,
    /// Pitch-accent rows imported (0 when the dictionary has none).
    pub pitches: usize,
}

/// Open (creating if needed) the user database and make sure its tables exist.
pub(crate) fn open_user_db(path: &Path) -> Result<Connection, Error> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v > USER_VERSION {
        return Err(Error::Format(format!(
            "user dictionary file is version {v}, this app understands {USER_VERSION}"
        )));
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dicts(id INTEGER PRIMARY KEY, title TEXT NOT NULL UNIQUE, revision TEXT NOT NULL,
             kind TEXT NOT NULL, terms INTEGER NOT NULL, freqs INTEGER NOT NULL, pitches INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS uterms(id INTEGER PRIMARY KEY, dict_id INTEGER NOT NULL, term TEXT NOT NULL,
             reading TEXT NOT NULL, reading_norm TEXT NOT NULL, score INTEGER NOT NULL, mask INTEGER NOT NULL, data TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS uterms_term ON uterms(term);
         CREATE INDEX IF NOT EXISTS uterms_reading ON uterms(reading_norm);
         CREATE INDEX IF NOT EXISTS uterms_dict ON uterms(dict_id);
         CREATE TABLE IF NOT EXISTS ufreq(dict_id INTEGER NOT NULL, term TEXT NOT NULL, reading TEXT NOT NULL, rank INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS ufreq_term ON ufreq(term, reading, rank);
         CREATE INDEX IF NOT EXISTS ufreq_dict ON ufreq(dict_id);
         CREATE TABLE IF NOT EXISTS upitch(dict_id INTEGER NOT NULL, term TEXT NOT NULL, reading TEXT NOT NULL, down TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS upitch_term ON upitch(term, reading);
         CREATE INDEX IF NOT EXISTS upitch_dict ON upitch(dict_id);",
    )?;
    conn.execute_batch(&format!("PRAGMA user_version = {USER_VERSION}"))?;
    Ok(conn)
}

/// Delete everything an earlier import of dictionary `id` added.
pub(crate) fn delete_dict(tx: &Transaction, id: i64) -> Result<(), Error> {
    for t in ["uterms", "ufreq", "upitch"] {
        tx.execute(&format!("DELETE FROM {t} WHERE dict_id = ?1"), [id])?;
    }
    tx.execute("DELETE FROM dicts WHERE id = ?1", [id])?;
    Ok(())
}

pub(crate) fn remove(path: &Path, title: &str) -> Result<(), Error> {
    let mut conn = open_user_db(path)?;
    let tx = conn.transaction()?;
    let id: Option<i64> = tx
        .query_row("SELECT id FROM dicts WHERE title = ?1", [title], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(id) = id {
        delete_dict(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}

/// Reject names that could escape a directory if anyone ever extracted the archive.
fn check_name(name: &str) -> Result<(), Error> {
    let evil = name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.starts_with('/')
        || name.split('/').any(|p| p == "..")
        || name.chars().nth(1) == Some(':');
    if evil {
        return Err(bad(format!("unsafe file name in zip: {name:?}")));
    }
    Ok(())
}

/// Stream the top-level JSON array of `r`, handing each element to `f`. Nesting deeper than serde_json's
/// limit (128) is an error, not a stack overflow.
fn for_each_row(r: impl Read, f: &mut dyn FnMut(Value) -> Result<(), Error>) -> Result<(), Error> {
    struct Rows<'a> {
        f: &'a mut dyn FnMut(Value) -> Result<(), Error>,
        err: &'a mut Option<Error>,
    }
    impl<'de> Visitor<'de> for Rows<'_> {
        type Value = ();
        fn expecting(&self, fm: &mut std::fmt::Formatter) -> std::fmt::Result {
            fm.write_str("an array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            while let Some(v) = seq.next_element::<Value>()? {
                if let Err(e) = (self.f)(v) {
                    *self.err = Some(e);
                    return Err(de::Error::custom("row rejected"));
                }
            }
            Ok(())
        }
    }
    let mut err = None;
    let mut de = serde_json::Deserializer::from_reader(BufReader::new(r));
    let res = de.deserialize_seq(Rows { f, err: &mut err });
    if let Some(e) = err {
        return Err(e);
    }
    res?;
    de.end()?;
    Ok(())
}

fn read_limited(r: impl Read, limit: u64) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    r.take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(bad("zip member is larger than allowed"));
    }
    Ok(buf)
}

fn text(v: &Value) -> Option<&str> {
    v.as_str()
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

struct Out {
    s: String,
    cap: usize,
}

impl Out {
    fn push(&mut self, t: &str) {
        for c in t.chars() {
            if self.s.chars().count() >= self.cap {
                return;
            }
            self.s.push(c);
        }
    }
    fn nl(&mut self) {
        if !self.s.ends_with('\n') && !self.s.is_empty() {
            self.s.push('\n');
        }
    }
}

/// Flatten Yomitan structured content into plain text; block elements become line breaks.
fn flatten(v: &Value, depth: usize, out: &mut Out) {
    if depth > MAX_FLATTEN_DEPTH {
        return;
    }
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => {
            for x in a {
                flatten(x, depth + 1, out);
            }
        }
        Value::Object(o) => {
            let tag = o.get("tag").and_then(Value::as_str).unwrap_or("");
            match o.get("type").and_then(Value::as_str) {
                Some("image") => return,
                Some("text") => {
                    if let Some(t) = o.get("text").and_then(Value::as_str) {
                        out.push(t);
                    }
                    return;
                }
                _ => {}
            }
            if matches!(tag, "rt" | "rp" | "img") {
                return;
            }
            let block = matches!(
                tag,
                "div" | "li" | "p" | "br" | "ul" | "ol" | "tr" | "table" | "details" | "summary"
            ) || tag.len() == 2 && tag.starts_with('h');
            if block {
                out.nl();
            }
            if let Some(c) = o.get("content") {
                flatten(c, depth + 1, out);
            } else if let Some(t) = o.get("text").and_then(Value::as_str) {
                out.push(t);
            }
            if block {
                out.nl();
            }
        }
        _ => {}
    }
}

fn glosses_of(glossary: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    let mut total = 0usize;
    for item in glossary {
        let mut o = Out {
            s: String::new(),
            cap: MAX_GLOSS_CHARS * 6,
        };
        flatten(item, 0, &mut o);
        for line in o.s.lines() {
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if line.is_empty() {
                continue;
            }
            let line = clip(&line, MAX_GLOSS_CHARS);
            total += line.chars().count();
            if out.len() >= MAX_GLOSSES || total > MAX_ENTRY_CHARS {
                return out;
            }
            out.push(line);
        }
    }
    out
}

/// Stored form of a term entry's senses (the `data` column).
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct UserSense {
    pub g: Vec<String>,
    pub p: Vec<String>,
    pub n: Vec<String>,
}

fn freq_value(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Object(o) => o.get("value").and_then(Value::as_f64),
        _ => None,
    }
}

fn read_index(bytes: &[u8]) -> Result<(String, String, bool), Error> {
    let v: Value = serde_json::from_slice(bytes)?;
    let o = v
        .as_object()
        .ok_or_else(|| bad("index.json is not an object"))?;
    let title = o
        .get("title")
        .and_then(text)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad("index.json has no title"))?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(bad("dictionary title is too long"));
    }
    let version = o
        .get("format")
        .or_else(|| o.get("version"))
        .and_then(Value::as_i64)
        .ok_or_else(|| bad("index.json has no format/version"))?;
    if !(2..=3).contains(&version) {
        return Err(bad(format!(
            "unsupported Yomitan dictionary format {version} (need 3)"
        )));
    }
    let revision = match o.get("revision") {
        Some(Value::String(s)) => clip(s, 100),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let occurrence = o.get("frequencyMode").and_then(Value::as_str) == Some("occurrence-based");
    Ok((title.to_string(), revision, occurrence))
}

#[derive(Clone, Copy, PartialEq)]
enum FileKind {
    Index,
    Terms,
    Meta,
    Other,
}

fn classify(name: &str) -> FileKind {
    if name.contains('/') {
        return FileKind::Other;
    }
    if name == "index.json" {
        return FileKind::Index;
    }
    let numbered = |prefix: &str| {
        name.strip_prefix(prefix)
            .and_then(|r| r.strip_suffix(".json"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    if numbered("term_bank_") {
        FileKind::Terms
    } else if numbered("term_meta_bank_") {
        FileKind::Meta
    } else {
        FileKind::Other
    }
}

/// Words that mean a plain "noun"-ish or verb-ish part of speech, in Yomitan's `rules` / tag vocabulary.
fn pos_mask_of(tokens: &[&str]) -> u32 {
    let mut m = deinflect::pos_mask(tokens.iter().copied());
    for t in tokens {
        match *t {
            "v5" => m |= V5ALL,
            "vs" => m |= VSN,
            _ => {}
        }
    }
    m as u32
}

pub(crate) fn import(zip_path: &Path, user_db: &Path) -> Result<ImportSummary, Error> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| bad(format!("not a readable zip: {e}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(bad("zip has too many files"));
    }
    // Pass 1: names and declared sizes, before anything is decompressed.
    let mut kinds: Vec<FileKind> = Vec::with_capacity(archive.len());
    let mut declared: u64 = 0;
    for i in 0..archive.len() {
        let f = archive
            .by_index_raw(i)
            .map_err(|e| bad(format!("bad zip entry: {e}")))?;
        let name = String::from_utf8_lossy(f.name_raw()).into_owned();
        check_name(&name)?;
        if f.is_dir() {
            kinds.push(FileKind::Other);
            continue;
        }
        let kind = classify(&name);
        if kind != FileKind::Other {
            if f.size() > MAX_FILE_BYTES {
                return Err(bad(format!("{} is larger than allowed", name)));
            }
            declared = declared.saturating_add(f.size());
            if declared > MAX_TOTAL_BYTES {
                return Err(bad("zip is larger than allowed when unpacked"));
            }
        }
        kinds.push(kind);
    }
    let index_pos = kinds
        .iter()
        .position(|k| *k == FileKind::Index)
        .ok_or_else(|| bad("not a Yomitan dictionary (no index.json at the top level)"))?;
    let bytes = {
        let f = archive
            .by_index(index_pos)
            .map_err(|e| bad(format!("bad zip entry: {e}")))?;
        read_limited(f, 10 << 20)?
    };
    let (title, revision, occurrence) = read_index(&bytes)?;

    let mut conn = open_user_db(user_db)?;
    let tx = conn.transaction()?;
    if let Some(old) = tx
        .query_row("SELECT id FROM dicts WHERE title = ?1", [&title], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
    {
        delete_dict(&tx, old)?;
    }
    tx.execute(
        "INSERT INTO dicts(title, revision, kind, terms, freqs, pitches) VALUES(?1, ?2, 'terms', 0, 0, 0)",
        params![title, revision],
    )?;
    let dict_id = tx.last_insert_rowid();

    let mut terms = 0usize;
    let mut pitches = 0usize;
    let mut freqs: Vec<(String, String, f64)> = Vec::new();
    let mut actual: u64 = 0;
    {
        let mut ins_term = tx.prepare(
            "INSERT INTO uterms(dict_id, term, reading, reading_norm, score, mask, data) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        )?;
        let mut ins_pitch =
            tx.prepare("INSERT INTO upitch(dict_id, term, reading, down) VALUES(?1,?2,?3,?4)")?;
        for (i, kind) in kinds.iter().enumerate() {
            if !matches!(kind, FileKind::Terms | FileKind::Meta) {
                continue;
            }
            let f = archive
                .by_index(i)
                .map_err(|e| bad(format!("bad zip entry: {e}")))?;
            let name = String::from_utf8_lossy(f.name_raw()).into_owned();
            let remaining = MAX_TOTAL_BYTES.saturating_sub(actual).min(MAX_FILE_BYTES);
            let counter = std::cell::Cell::new(0u64);
            let reader = CountingTake {
                inner: f,
                left: remaining + 1,
                counter: &counter,
            };
            let ctx = |e: Error| match e {
                Error::Format(m) => bad(format!("{name}: {m}")),
                other => other,
            };
            if *kind == FileKind::Terms {
                for_each_row(reader, &mut |row| {
                    if terms + freqs.len() + pitches >= MAX_ROWS {
                        return Err(bad("too many rows"));
                    }
                    if let Some((t, r, score, mask, data)) = term_row(&row)? {
                        ins_term.execute(params![
                            dict_id,
                            t,
                            r,
                            to_hiragana(&r),
                            score,
                            mask,
                            data
                        ])?;
                        terms += 1;
                    }
                    Ok(())
                })
                .map_err(ctx)?;
            } else {
                for_each_row(reader, &mut |row| {
                    if terms + freqs.len() + pitches >= MAX_ROWS {
                        return Err(bad("too many rows"));
                    }
                    meta_row(&row, &mut freqs, &mut |t, r, d| {
                        pitches += 1;
                        ins_pitch.execute(params![dict_id, t, to_hiragana(r), d])?;
                        Ok(())
                    })
                })
                .map_err(ctx)?;
            }
            actual += counter.get();
            if actual > MAX_TOTAL_BYTES {
                return Err(bad("zip is larger than allowed when unpacked"));
            }
        }
    }

    // Occurrence counts become ranks: the highest count is rank 1, ties share a rank.
    if occurrence {
        freqs.sort_by(|a, b| b.2.total_cmp(&a.2));
        let mut rank = 0u64;
        let mut last = f64::NAN;
        for (i, f) in freqs.iter_mut().enumerate() {
            if f.2 != last {
                rank = i as u64 + 1;
                last = f.2;
            }
            f.2 = rank as f64;
        }
    }
    {
        let mut ins =
            tx.prepare("INSERT INTO ufreq(dict_id, term, reading, rank) VALUES(?1,?2,?3,?4)")?;
        for (t, r, v) in &freqs {
            let rank = v.round().clamp(1.0, f64::from(u32::MAX)) as i64;
            ins.execute(params![dict_id, t, r, rank])?;
        }
    }
    let kind = match (terms > 0, !freqs.is_empty(), pitches > 0) {
        (true, true, _) => ImportKind::Both,
        (true, false, _) => ImportKind::Terms,
        (false, true, _) => ImportKind::Frequency,
        (false, false, true) => ImportKind::Pitch,
        (false, false, false) => {
            return Err(bad("the dictionary has no term, frequency or pitch data"));
        }
    };
    tx.execute(
        "UPDATE dicts SET kind = ?2, terms = ?3, freqs = ?4, pitches = ?5 WHERE id = ?1",
        params![dict_id, kind.as_str(), terms, freqs.len(), pitches],
    )?;
    tx.commit()?;
    Ok(ImportSummary {
        title,
        revision,
        kind,
        terms,
        frequencies: freqs.len(),
        pitches,
    })
}

/// `Take` that also reports how many bytes were really read.
struct CountingTake<'a, R> {
    inner: R,
    left: u64,
    counter: &'a std::cell::Cell<u64>,
}

impl<R: Read> Read for CountingTake<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Err(std::io::Error::other("zip member is larger than allowed"));
        }
        let n = buf.len().min(self.left.min(usize::MAX as u64) as usize);
        let got = self.inner.read(&mut buf[..n])?;
        self.left -= got as u64;
        self.counter.set(self.counter.get() + got as u64);
        Ok(got)
    }
}

type TermRow = (String, String, i64, u32, String);

/// One `term_bank` row -> (term, reading, score, pos mask, json senses); `None` for an empty expression.
fn term_row(row: &Value) -> Result<Option<TermRow>, Error> {
    let a = row
        .as_array()
        .ok_or_else(|| bad("term row is not an array"))?;
    if a.len() < 6 {
        return Err(bad("term row has fewer than 6 columns"));
    }
    let expr = a[0].as_str().ok_or_else(|| bad("term is not a string"))?;
    let reading = match &a[1] {
        Value::String(s) => s.as_str(),
        Value::Null => "",
        _ => return Err(bad("reading is not a string")),
    };
    let tags_str = |v: &Value| -> Result<String, Error> {
        match v {
            Value::String(s) => Ok(s.clone()),
            Value::Null => Ok(String::new()),
            _ => Err(bad("tags are not a string")),
        }
    };
    let def_tags = tags_str(&a[2])?;
    let rules = tags_str(&a[3])?;
    let score = a[4]
        .as_f64()
        .ok_or_else(|| bad("score is not a number"))?
        .clamp(-1e9, 1e9) as i64;
    let gloss = a[5]
        .as_array()
        .ok_or_else(|| bad("glossary is not an array"))?;
    if expr.is_empty() || expr.chars().count() > MAX_TERM_CHARS {
        return Ok(None);
    }
    let term_tags = a.get(7).map(tags_str).transpose()?.unwrap_or_default();
    let reading = if reading.is_empty() { expr } else { reading };
    let mut pos: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut tokens: Vec<&str> = Vec::new();
    for t in def_tags
        .split_whitespace()
        .chain(rules.split_whitespace())
        .chain(term_tags.split_whitespace())
    {
        if tokens.len() < 64 && t.chars().count() <= 40 {
            tokens.push(t);
        }
    }
    for t in &tokens {
        if is_pos_code(t) {
            if !pos.iter().any(|p| p == t) {
                pos.push((*t).to_string());
            }
        } else if !notes.iter().any(|n| n == t) {
            notes.push((*t).to_string());
        }
    }
    let sense = UserSense {
        g: glosses_of(gloss),
        p: pos,
        n: notes,
    };
    if sense.g.is_empty() {
        return Ok(None);
    }
    let data = serde_json::to_string(&[&sense])?;
    Ok(Some((
        expr.to_string(),
        clip(reading, MAX_TERM_CHARS),
        score,
        pos_mask_of(&tokens),
        data,
    )))
}

/// Does `t` look like a JMdict part-of-speech code (the vocabulary Yomitan dictionaries borrow)?
fn is_pos_code(t: &str) -> bool {
    matches!(
        t,
        "n" | "v1"
            | "v5"
            | "vs"
            | "vk"
            | "vt"
            | "vi"
            | "adj-i"
            | "adj-na"
            | "adj-no"
            | "adj-pn"
            | "adj-ix"
            | "adv"
            | "prt"
            | "exp"
            | "int"
            | "cop"
            | "conj"
            | "pn"
            | "ctr"
            | "pref"
            | "suf"
            | "aux"
            | "aux-v"
            | "aux-adj"
            | "num"
            | "vs-i"
            | "vs-s"
            | "vs-c"
    ) || t.starts_with("v5")
}

/// One `term_meta_bank` row: frequencies are pushed to `freqs`, pitch rows go to `pitch(term, reading, "0,2")`.
fn meta_row(
    row: &Value,
    freqs: &mut Vec<(String, String, f64)>,
    pitch: &mut dyn FnMut(&str, &str, &str) -> Result<(), Error>,
) -> Result<(), Error> {
    let a = row
        .as_array()
        .ok_or_else(|| bad("term meta row is not an array"))?;
    if a.len() < 3 {
        return Err(bad("term meta row has fewer than 3 columns"));
    }
    let expr = a[0].as_str().ok_or_else(|| bad("term is not a string"))?;
    let mode = a[1].as_str().ok_or_else(|| bad("mode is not a string"))?;
    if expr.is_empty() || expr.chars().count() > MAX_TERM_CHARS {
        return Ok(());
    }
    match mode {
        "freq" => {
            let (reading, v) = match &a[2] {
                Value::Object(o) if o.contains_key("frequency") => (
                    o.get("reading").and_then(Value::as_str).unwrap_or(""),
                    o.get("frequency").and_then(freq_value),
                ),
                other => ("", freq_value(other)),
            };
            let v = v.ok_or_else(|| bad("frequency has no numeric value"))?;
            if v.is_finite() && v > 0.0 {
                freqs.push((expr.to_string(), to_hiragana(reading), v));
            }
        }
        "pitch" => {
            let o = a[2]
                .as_object()
                .ok_or_else(|| bad("pitch data is not an object"))?;
            let reading = o
                .get("reading")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("pitch data has no reading"))?;
            let list = o
                .get("pitches")
                .and_then(Value::as_array)
                .ok_or_else(|| bad("pitch data has no pitches"))?;
            let mut downs: Vec<u32> = Vec::new();
            for p in list {
                if let Some(n) = p
                    .get("position")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    && n < 100
                    && !downs.contains(&n)
                {
                    downs.push(n);
                }
            }
            if !downs.is_empty() {
                let s: Vec<String> = downs.iter().map(u32::to_string).collect();
                pitch(expr, reading, &s.join(","))?;
            }
        }
        _ => {}
    }
    Ok(())
}
