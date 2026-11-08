//! The compact per-entry record stored as JSON in the `entries` table (short keys keep the database small).

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "3";

fn is_false(b: &bool) -> bool {
    !*b
}

/// One written form (kanji) or reading (kana) of an entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Form {
    /// Text.
    pub t: String,
    /// JMdict "common" flag (1 = common).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub c: u8,
    /// Raw JMdict tags (sK, rK, ateji...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub g: Vec<String>,
    /// For readings: indices into `k` of the kanji forms it applies to (empty = all).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub a: Vec<u16>,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl Form {
    pub fn common(&self) -> bool {
        self.c != 0
    }

    /// Search-only forms (sK/sk) are matched but never displayed as the headword.
    pub fn search_only(&self) -> bool {
        self.g.iter().any(|t| t == "sK" || t == "sk")
    }

    /// Rarely used, outdated or irregular forms.
    pub fn rare(&self) -> bool {
        self.g
            .iter()
            .any(|t| matches!(t.as_str(), "rK" | "iK" | "oK" | "ik" | "io" | "ok" | "rk"))
    }
}

/// One meaning.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SenseRec {
    /// Glosses.
    pub g: Vec<String>,
    /// Part-of-speech tag codes (labels live in the `tags` table). Empty = same as the previous sense.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub p: Vec<String>,
    /// Misc / field / dialect tag codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub n: Vec<String>,
    /// Free-text usage info.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub i: Vec<String>,
    /// Restricted to these kanji forms (indices into `k`; empty = all).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ak: Vec<u16>,
    /// Restricted to these readings (indices into `r`; empty = all).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ar: Vec<u16>,
    /// "usually written using kana alone".
    #[serde(default, skip_serializing_if = "is_false")]
    pub u: bool,
}

/// A whole entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stored {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<Form>,
    pub r: Vec<Form>,
    pub s: Vec<SenseRec>,
}

impl Stored {
    /// Part-of-speech tag codes of every sense, with the "same as previous" gaps filled in.
    pub fn resolved_pos(&self) -> Vec<&[String]> {
        let mut last: &[String] = &[];
        self.s
            .iter()
            .map(|s| {
                if !s.p.is_empty() {
                    last = &s.p;
                }
                last
            })
            .collect()
    }

    /// Union of all part-of-speech tag codes (for the deinflection check).
    pub fn pos_tags(&self) -> impl Iterator<Item = &str> {
        self.s.iter().flat_map(|s| s.p.iter().map(String::as_str))
    }
}
