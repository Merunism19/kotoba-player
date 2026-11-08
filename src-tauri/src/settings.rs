//! App settings stored through the stores registry (`settings.json`): Anki deck and tags, translation and audio choices.

use serde::{Deserialize, Serialize};

/// Deck used until the user picks another one (created on first card if missing).
pub const DEFAULT_DECK: &str = "Kotoba Mining";

/// Everything the user can set. Secrets (the DeepL key) are NOT here; they live in the OS keyring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub anki_deck: String,
    /// Extra tags on every mined card (the app always adds "kotoba" and the show's tag).
    pub anki_tags: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            anki_deck: DEFAULT_DECK.to_string(),
            anki_tags: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_tolerant_json() {
        let s: Settings = serde_json::from_str(r#"{"unknown": 1}"#).unwrap();
        assert_eq!(s, Settings::default());
        let s: Settings = serde_json::from_str(r#"{"ankiDeck": "Japanese::Anime"}"#).unwrap();
        assert_eq!(s.anki_deck, "Japanese::Anime");
    }
}
