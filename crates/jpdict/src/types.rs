//! The JSON the front end receives. Field names and optionality mirror `src/lib/backend.ts`.

use serde::Serialize;

/// Pitch accent of one reading (Tokyo dialect, from Kanjium).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pitch {
    /// Mora after which the pitch drops; 0 = no drop (heiban).
    pub downstep: u32,
    /// `heiban`, `atamadaka`, `nakadaka` or `odaka`.
    pub pattern: &'static str,
    pub morae: Vec<String>,
}

/// One meaning of an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sense {
    pub glosses: Vec<String>,
    pub pos: Vec<String>,
    pub notes: Vec<String>,
}

/// One furigana segment: `ruby` is absent for plain kana.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Furigana {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<String>,
}

/// A dictionary entry as shown in the popup.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntry {
    /// JMdict sequence number.
    pub id: i64,
    pub term: String,
    pub reading: String,
    pub furigana: Vec<Furigana>,
    pub senses: Vec<Sense>,
    pub pitch: Vec<Pitch>,
    pub common: bool,
    /// Approximate frequency rank; absent when unknown (the jmdict-simplified file carries no nfXX tags).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freq_rank: Option<u32>,
    pub other_forms: Vec<String>,
}

/// Result of looking up the text at a position; one per matched length, longest first.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResult {
    /// Offset (code points) of the match in the line.
    pub start: usize,
    /// Length (code points) of the matched text.
    pub len: usize,
    pub matched: String,
    /// Dictionary form it was reduced to, when it was conjugated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deinflected: Option<String>,
    /// Conjugation steps, innermost first, outermost (the form as written) last.
    pub inflections: Vec<String>,
    pub entries: Vec<DictEntry>,
}
