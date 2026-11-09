//! Opening the database and looking words up at a position in a line.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use rusqlite::{Connection, OpenFlags, params};

use crate::deinflect::{self, Candidate, Mask, START, VS, VSN};
use crate::error::Error;
use crate::furigana::furigana;
use crate::kana::{has_katakana, is_word_char, to_hiragana};
use crate::pitch::{parse_downsteps, pitch_for};
use crate::store::{Form, SCHEMA_VERSION, SenseRec, Stored};
use crate::types::{DictEntry, LookupResult, Pitch, Sense};
use crate::user::{self, ImportKind, ImportSummary, UserSense};

const U_TERMS: u8 = 1;
const U_FREQ: u8 = 2;
const U_PITCH: u8 = 4;

/// Hits whose frequency is looked up for ranking (a short kana string can match hundreds).
const MAX_RANKED_HITS: usize = 120;
/// Result slots kept for user-dictionary entries when JMdict alone would fill the list.
const MAX_USER_RESERVED: usize = 5;

/// Longest text (in code points) a lookup tries to match.
pub const MAX_MATCH_CHARS: usize = 20;
/// Entries returned per result; short kana strings can match hundreds.
pub const MAX_ENTRIES_PER_RESULT: usize = 25;

/// A read-only handle on the dictionary database. `Send + Sync`: calls take turns on one connection
/// (a lookup takes well under a millisecond, so a pool would only add memory).
pub struct Dict {
    conn: Mutex<Connection>,
    /// Tag code -> plain-English label.
    tags: HashMap<String, String>,
    /// The attached user database file, if any.
    user_path: Mutex<Option<PathBuf>>,
    /// Which kinds of user data exist (`U_*` bits), so lookups skip empty tables.
    user_flags: AtomicU8,
}

/// A deinflection candidate prepared for the database.
struct Cand {
    /// The dictionary form as the rules produced it (what the user sees as "deinflected").
    word: String,
    /// The text actually looked up (differs from `word` for suru-nouns: 勉強する -> 勉強).
    lookup: String,
    mask: Mask,
    chain: Vec<&'static str>,
}

struct UserHit {
    cand: usize,
    score: i64,
    entry: DictEntry,
}

struct Hit {
    id: i64,
    kind: i64,
    idx: usize,
    cand: usize,
    key: (u8, u8, u8, u32, usize, i64),
}

impl Dict {
    /// Open the database at `path` read-only.
    pub fn open(path: impl AsRef<Path>) -> Result<Dict, Error> {
        let conn = Connection::open_with_flags(
            path.as_ref(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| {
                Error::Format("not a Kotoba Player dictionary (no schema_version)".into())
            })?;
        if version != SCHEMA_VERSION {
            return Err(Error::Format(format!(
                "dictionary schema {version}, expected {SCHEMA_VERSION}; rebuild it"
            )));
        }
        conn.execute_batch(
            "PRAGMA query_only = ON; PRAGMA cache_size = -16384; PRAGMA mmap_size = 268435456;",
        )?;
        let tags: HashMap<String, String> = {
            let mut stmt = conn.prepare("SELECT code, label FROM tags")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?
        };
        Ok(Dict {
            conn: Mutex::new(conn),
            tags,
            user_path: Mutex::new(None),
            user_flags: AtomicU8::new(0),
        })
    }

    /// Attach the user database at `user_db` (created if missing); later lookups include its terms,
    /// frequencies and pitch accents. Replaces an earlier attachment.
    pub fn attach_user_db(&self, user_db: &Path) -> Result<(), Error> {
        drop(user::open_user_db(user_db)?);
        let path = user_db
            .to_str()
            .ok_or_else(|| Error::Format("user database path is not valid UTF-8".into()))?;
        {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            let _ = conn.execute_batch("DETACH DATABASE u");
            self.user_flags.store(0, Ordering::Relaxed);
            conn.execute("ATTACH DATABASE ?1 AS u", [path])?;
        }
        *self.user_path.lock().unwrap_or_else(|p| p.into_inner()) = Some(user_db.to_path_buf());
        self.refresh_user_flags();
        Ok(())
    }

    fn refresh_user_flags(&self) {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let sums: Result<(i64, i64, i64), _> = conn.query_row(
            "SELECT COALESCE(SUM(terms),0), COALESCE(SUM(freqs),0), COALESCE(SUM(pitches),0) FROM u.dicts",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        );
        let (t, f, p) = sums.unwrap_or((0, 0, 0));
        let flags =
            (u8::from(t > 0) * U_TERMS) | (u8::from(f > 0) * U_FREQ) | (u8::from(p > 0) * U_PITCH);
        self.user_flags.store(flags, Ordering::Relaxed);
    }

    /// Import a Yomitan-format `.zip` into the user database at `user_db` (created if missing), in one
    /// transaction; an earlier import with the same title is replaced. The database is attached for
    /// lookups if it was not already.
    pub fn import_yomitan(&self, zip: &Path, user_db: &Path) -> Result<ImportSummary, Error> {
        let summary = user::import(zip, user_db)?;
        let attached = self
            .user_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_deref()
            == Some(user_db);
        if attached {
            self.refresh_user_flags();
        } else {
            self.attach_user_db(user_db)?;
        }
        Ok(summary)
    }

    /// The imported user dictionaries: (title, kind, rows).
    pub fn user_dictionaries(&self) -> Vec<(String, ImportKind, usize)> {
        if self
            .user_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_none()
        {
            return Vec::new();
        }
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let Ok(mut stmt) =
            conn.prepare("SELECT title, kind, terms + freqs + pitches FROM u.dicts ORDER BY id")
        else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        });
        match rows {
            Ok(rows) => rows
                .filter_map(Result::ok)
                .map(|(t, k, n)| (t, ImportKind::parse(&k), n.max(0) as usize))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Remove an imported dictionary and all its rows (no error when there is none of that title).
    pub fn remove_user_dictionary(&self, title: &str) -> Result<(), Error> {
        let path = self
            .user_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| Error::Format("no user database attached".into()))?;
        user::remove(&path, title)?;
        self.refresh_user_flags();
        Ok(())
    }

    /// A value from the `meta` table (versions, attribution).
    pub fn meta(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    /// Look up the word that starts at code-point `offset` of `text`.
    ///
    /// The scan always starts at `offset` (the clicked character, or the start of the unit under the
    /// cursor): it never looks backwards. Substrings from the longest (at most [`MAX_MATCH_CHARS`],
    /// and never across punctuation or spaces) down to one character are deinflected and looked up;
    /// each length that has entries yields one [`LookupResult`], longest first. An entry appears only in
    /// the longest result that found it. Entries are ranked: exact written form before deinflected,
    /// then common words, then rarity of the matched form, then frequency, then JMdict order.
    /// Returns nothing when `offset` is past the end or points at punctuation. Never panics on odd input.
    pub fn lookup_at(&self, text: &str, offset: usize) -> Vec<LookupResult> {
        let chars: Vec<char> = text.chars().collect();
        if offset >= chars.len() {
            return Vec::new();
        }
        let mut max = 0;
        while max < MAX_MATCH_CHARS
            && offset + max < chars.len()
            && is_word_char(chars[offset + max])
        {
            max += 1;
        }
        if max == 0 {
            return Vec::new();
        }
        // A panic in an earlier lookup must not disable lookups for the rest of the session.
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        // One read transaction for the whole lookup: SQLite then takes the file lock once instead of once
        // per statement (about 30 microseconds each on Windows, which dominated the lookup time).
        let Ok(tx) = conn.unchecked_transaction() else {
            return Vec::new();
        };
        self.scan(&tx, &chars, offset, max).unwrap_or_default()
    }

    fn scan(
        &self,
        conn: &Connection,
        chars: &[char],
        offset: usize,
        max: usize,
    ) -> Result<Vec<LookupResult>, Error> {
        let flags = self.user_flags.load(Ordering::Relaxed);
        let (has_terms, has_freq, has_pitch) = (
            flags & U_TERMS != 0,
            flags & U_FREQ != 0,
            flags & U_PITCH != 0,
        );
        let mut cache: HashMap<i64, Rc<Stored>> = HashMap::new();
        let mut claimed: HashSet<i64> = HashSet::new();
        let mut results = Vec::new();
        let mut forms_stmt =
            conn.prepare_cached("SELECT entry_id, kind, idx, pri FROM forms WHERE text = ?1")?;

        for len in (1..=max).rev() {
            let sub: String = chars[offset..offset + len].iter().collect();
            let cands = candidates(&sub);
            let mut hits: Vec<Hit> = Vec::new();
            for (ci, c) in cands.iter().enumerate() {
                let rows: Vec<(i64, i64, i64, i64)> = forms_stmt
                    .query_map([&c.lookup], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                    })?
                    .collect::<Result<_, _>>()?;
                for (id, kind, idx, pri) in rows {
                    if !c.chain.is_empty() {
                        // A deinflected form is only real when the entry's part of speech agrees.
                        let st = load(conn, &mut cache, id)?;
                        if deinflect::pos_mask(st.pos_tags()) & c.mask == 0 {
                            continue;
                        }
                    }
                    hits.push(Hit {
                        id,
                        kind,
                        idx: idx as usize,
                        cand: ci,
                        key: (
                            u8::from(!c.chain.is_empty()),
                            (pri >> 1) as u8,
                            (pri & 1) as u8,
                            u32::MAX,
                            idx as usize,
                            id,
                        ),
                    });
                }
            }
            let uhits = if has_terms {
                user_hits(conn, &cands, &self.tags)?
            } else {
                Vec::new()
            };
            if hits.is_empty() && uhits.is_empty() {
                continue;
            }
            hits.sort_by_key(|h| h.key);
            if has_freq {
                // Known frequencies reorder the best candidates (the rest keep JMdict order).
                for h in hits.iter_mut().take(MAX_RANKED_HITS) {
                    let st = load(conn, &mut cache, h.id)?;
                    let (kidx, ridx) = choose(&st, h.kind, h.idx);
                    let reading = &st.r[ridx].t;
                    let term = kidx.map_or(reading, |k| &st.k[k].t);
                    if let Some(f) = freq_of(conn, term, reading) {
                        h.key.3 = f;
                    }
                }
                hits.sort_by_key(|h| h.key);
            }
            let mut seen_here: HashSet<i64> = HashSet::new();
            let mut chosen: Vec<&Hit> = Vec::new();
            for h in &hits {
                if !claimed.contains(&h.id) && seen_here.insert(h.id) {
                    chosen.push(h);
                }
            }
            claimed.extend(seen_here);
            let mut uhits: Vec<UserHit> = uhits
                .into_iter()
                .filter(|u| !claimed.contains(&u.entry.id))
                .collect();
            for u in &mut uhits {
                u.entry.freq_rank = if has_freq {
                    freq_of(conn, &u.entry.term, &u.entry.reading)
                } else {
                    None
                };
                u.entry.pitch = pitch_of(conn, None, &u.entry.term, &u.entry.reading, has_pitch);
            }
            uhits.sort_by_key(|u| {
                (
                    u8::from(!cands[u.cand].chain.is_empty()),
                    u.entry.freq_rank.unwrap_or(u32::MAX),
                    std::cmp::Reverse(u.score),
                    u.entry.id,
                )
            });
            let top_cand = match (chosen.first(), uhits.first()) {
                (Some(h), _) => h.cand,
                (None, Some(u)) => u.cand,
                (None, None) => continue,
            };
            let top = &cands[top_cand];
            let user_room = uhits.len().min(MAX_USER_RESERVED);
            let mut entries: Vec<DictEntry> = Vec::new();
            for h in chosen
                .iter()
                .take(MAX_ENTRIES_PER_RESULT.saturating_sub(user_room))
            {
                let st = load(conn, &mut cache, h.id)?;
                entries.push(build_entry(
                    conn, &self.tags, h.id, &st, h.kind, h.idx, flags,
                ));
            }
            // User entries come after the JMdict ones; a copy of a JMdict entry (same term, reading and
            // glosses) is dropped.
            let glosses_of = |e: &DictEntry| -> Vec<String> {
                e.senses
                    .iter()
                    .flat_map(|s| s.glosses.iter().cloned())
                    .collect()
            };
            let jm: Vec<(String, String, Vec<String>)> = entries
                .iter()
                .map(|e| (e.term.clone(), e.reading.clone(), glosses_of(e)))
                .collect();
            for u in uhits {
                if entries.len() >= MAX_ENTRIES_PER_RESULT {
                    break;
                }
                let g = glosses_of(&u.entry);
                if jm
                    .iter()
                    .any(|(t, r, jg)| *t == u.entry.term && *r == u.entry.reading && *jg == g)
                {
                    continue;
                }
                claimed.insert(u.entry.id);
                entries.push(u.entry);
            }
            if entries.is_empty() {
                continue;
            }
            results.push(LookupResult {
                start: offset,
                len,
                matched: sub,
                deinflected: (!top.chain.is_empty()).then(|| top.word.clone()),
                inflections: top.chain.iter().map(|s| s.to_string()).collect(),
                entries,
            });
        }
        Ok(results)
    }
}

fn load(
    conn: &Connection,
    cache: &mut HashMap<i64, Rc<Stored>>,
    id: i64,
) -> Result<Rc<Stored>, Error> {
    if let Some(s) = cache.get(&id) {
        return Ok(s.clone());
    }
    let data: String = conn
        .prepare_cached("SELECT data FROM entries WHERE id = ?1")?
        .query_row([id], |r| r.get(0))?;
    let s: Rc<Stored> = Rc::new(serde_json::from_str(&data)?);
    cache.insert(id, s.clone());
    Ok(s)
}

/// Deinflection candidates for `sub` (and for its hiragana form when it has katakana), plus the
/// suru-noun reading of any "...する" result.
fn candidates(sub: &str) -> Vec<Cand> {
    let mut all: Vec<Candidate> = deinflect::deinflect(sub);
    if has_katakana(sub) {
        let hira = to_hiragana(sub);
        all.extend(deinflect::deinflect(&hira));
    }
    let mut out: Vec<Cand> = Vec::new();
    let mut seen: HashSet<(String, Mask)> = HashSet::new();
    for c in all {
        if !seen.insert((c.word.clone(), c.mask)) {
            continue;
        }
        if c.mask & VS != 0
            && let Some(noun) = c.word.strip_suffix("する")
            && !noun.is_empty()
            && seen.insert((noun.to_string(), VSN))
        {
            out.push(Cand {
                word: c.word.clone(),
                lookup: noun.to_string(),
                mask: VSN,
                chain: c.chain.clone(),
            });
        }
        let mask = if c.chain.is_empty() { START } else { c.mask };
        out.push(Cand {
            lookup: c.word.clone(),
            word: c.word,
            mask,
            chain: c.chain,
        });
    }
    out
}

/// `restrict` (indices; empty = no restriction) allows form `i`.
fn allows(restrict: &[u16], i: usize) -> bool {
    restrict.is_empty() || restrict.contains(&(i as u16))
}

/// Pick the headword and reading to show for a match on form `idx` of kind `kind`:
/// (index of the kanji form if any, index of the reading).
fn choose(st: &Stored, kind: i64, idx: usize) -> (Option<usize>, usize) {
    // Best reading for a kanji form: not search-only, applicable, preferably not rare.
    let reading_for = |k: usize| -> usize {
        let ok = |(_, r): &(usize, &Form)| !r.search_only() && allows(&r.a, k);
        let all = || st.r.iter().enumerate();
        all()
            .filter(ok)
            .find(|(_, r)| !r.rare())
            .or_else(|| all().find(ok))
            .map_or(0, |(i, _)| i)
    };
    // Best kanji form for a reading.
    let kanji_for = |r: usize| -> Option<usize> {
        let ok = |(i, k): &(usize, &Form)| !k.search_only() && allows(&st.r[r].a, *i);
        let all = || st.k.iter().enumerate();
        all()
            .filter(ok)
            .find(|(_, k)| !k.rare())
            .or_else(|| all().find(ok))
            .map(|(i, _)| i)
    };
    // A search-only form is never the headword: show the best normal form of the same kind instead.
    let displayable = |forms: &[Form], i: usize| -> usize {
        if !forms[i].search_only() {
            return i;
        }
        let all = || forms.iter().enumerate();
        all()
            .find(|(_, f)| !f.search_only() && !f.rare())
            .or_else(|| all().find(|(_, f)| !f.search_only()))
            .map_or(i, |(j, _)| j)
    };
    if kind == 0 {
        let k = displayable(&st.k, idx);
        (Some(k), reading_for(k))
    } else {
        let r = displayable(&st.r, idx);
        let usually_kana = !st.s.is_empty() && st.s.iter().all(|s| s.u);
        (if usually_kana { None } else { kanji_for(r) }, r)
    }
}

fn build_entry(
    conn: &Connection,
    tags: &HashMap<String, String>,
    id: i64,
    st: &Stored,
    kind: i64,
    idx: usize,
    flags: u8,
) -> DictEntry {
    let (has_freq, has_pitch) = (flags & U_FREQ != 0, flags & U_PITCH != 0);
    let (kidx, ridx) = choose(st, kind, idx);
    let reading = st.r[ridx].t.clone();
    let term = kidx.map_or_else(|| reading.clone(), |k| st.k[k].t.clone());
    let label = |code: &String| tags.get(code).cloned().unwrap_or_else(|| code.clone());

    let pos = st.resolved_pos();
    let all: Vec<(&SenseRec, &[String])> = st.s.iter().zip(pos).collect();
    let shown: Vec<_> = all
        .iter()
        .filter(|(s, _)| kidx.is_none_or(|k| allows(&s.ak, k)) && allows(&s.ar, ridx))
        .copied()
        .collect();
    let senses = if shown.is_empty() { all } else { shown };

    let mut other_forms: Vec<String> = Vec::new();
    for f in st.k.iter().chain(st.r.iter()) {
        if !f.search_only() && f.t != term && f.t != reading && !other_forms.contains(&f.t) {
            other_forms.push(f.t.clone());
        }
    }
    let common = kidx.is_some_and(|k| st.k[k].common()) || st.r[ridx].common();

    DictEntry {
        id,
        furigana: furigana(&term, &reading),
        pitch: pitch_of(conn, Some(st), &term, &reading, has_pitch),
        senses: senses
            .into_iter()
            .map(|(s, p)| {
                let mut notes: Vec<String> = Vec::new();
                for n in s.n.iter().map(label).chain(s.i.iter().cloned()) {
                    if !notes.contains(&n) {
                        notes.push(n);
                    }
                }
                Sense {
                    glosses: s.g.clone(),
                    pos: p.iter().map(label).collect(),
                    notes,
                }
            })
            .collect(),
        common,
        freq_rank: if has_freq {
            freq_of(conn, &term, &reading)
        } else {
            None
        },
        other_forms,
        term,
        reading,
    }
}

/// Best (lowest) rank of (term, reading) over the imported frequency dictionaries: the exact pair first,
/// else the term alone.
fn freq_of(conn: &Connection, term: &str, reading: &str) -> Option<u32> {
    let norm = to_hiragana(reading);
    let min = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Option<u32> {
        conn.prepare_cached(sql)
            .ok()?
            .query_row(p, |r| r.get::<_, Option<i64>>(0))
            .ok()
            .flatten()
            .map(|n| n.clamp(1, i64::from(u32::MAX)) as u32)
    };
    min(
        "SELECT MIN(rank) FROM u.ufreq WHERE term = ?1 AND (reading = ?2 OR reading = '')",
        &[&term, &norm],
    )
    .or_else(|| min("SELECT MIN(rank) FROM u.ufreq WHERE term = ?1", &[&term]))
}

type UserRow = (i64, String, String, i64, i64, String);

/// User term entries matching any candidate. Ids are negative (minus the row id) so they never clash
/// with JMdict sequence numbers.
fn user_hits(
    conn: &Connection,
    cands: &[Cand],
    tags: &HashMap<String, String>,
) -> Result<Vec<UserHit>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, term, reading, score, mask, data FROM u.uterms WHERE term = ?1 OR reading_norm = ?2",
    )?;
    let mut out: Vec<UserHit> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    for (ci, c) in cands.iter().enumerate() {
        let norm = to_hiragana(&c.lookup);
        let rows: Vec<UserRow> = stmt
            .query_map(params![c.lookup, norm], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        for (rid, term, reading, score, mask, data) in rows {
            if !c.chain.is_empty() && (mask as u32) & c.mask == 0 {
                continue;
            }
            if !seen.insert(rid) {
                continue;
            }
            let Ok(senses) = serde_json::from_str::<Vec<UserSense>>(&data) else {
                continue;
            };
            let label = |code: &String| tags.get(code).cloned().unwrap_or_else(|| code.clone());
            out.push(UserHit {
                cand: ci,
                score,
                entry: DictEntry {
                    id: -rid,
                    furigana: furigana(&term, &reading),
                    pitch: Vec::new(),
                    senses: senses
                        .into_iter()
                        .map(|s| Sense {
                            glosses: s.g,
                            pos: s.p.iter().map(label).collect(),
                            notes: s.n,
                        })
                        .collect(),
                    common: false,
                    freq_rank: None,
                    other_forms: Vec::new(),
                    term,
                    reading,
                },
            });
        }
    }
    Ok(out)
}

/// Kanjium accents for (term, reading), then for the entry's other kanji forms; for kana terms
/// finally by reading alone.
fn pitch_of(
    conn: &Connection,
    st: Option<&Stored>,
    term: &str,
    reading: &str,
    user: bool,
) -> Vec<Pitch> {
    let norm = to_hiragana(reading);
    let one = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Option<String> {
        conn.prepare_cached(sql)
            .ok()?
            .query_row(p, |r| r.get(0))
            .ok()
    };
    let by_pair = |t: &str| -> Option<String> {
        one(
            "SELECT down FROM pitch WHERE term = ?1 AND reading = ?2",
            &[&t, &norm],
        )
        .or_else(|| {
            if user {
                one(
                    "SELECT down FROM u.upitch WHERE term = ?1 AND reading = ?2",
                    &[&t, &norm],
                )
            } else {
                None
            }
        })
    };
    let mut down = by_pair(term);
    if down.is_none()
        && let Some(st) = st
    {
        down =
            st.k.iter()
                .filter(|k| k.t != term && !k.search_only())
                .find_map(|k| by_pair(&k.t));
    }
    if down.is_none() && !term.chars().any(crate::kana::is_kanji) {
        down = one(
            "SELECT down FROM pitch WHERE reading = ?1 LIMIT 1",
            &[&norm],
        )
        .or_else(|| {
            if user {
                one(
                    "SELECT down FROM u.upitch WHERE reading = ?1 LIMIT 1",
                    &[&norm],
                )
            } else {
                None
            }
        });
    }
    down.map(|d| {
        parse_downsteps(&d)
            .into_iter()
            .filter_map(|n| pitch_for(reading, n))
            .collect()
    })
    .unwrap_or_default()
}
