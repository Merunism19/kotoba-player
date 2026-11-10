//! App settings stored through the stores registry (`settings.json`): Anki, translation, subtitles and appearance.

use serde::{Deserialize, Serialize};

/// Deck used until the user picks another one (created on first card if missing).
pub const DEFAULT_DECK: &str = "Kotoba Mining";

/// Everything the user can set (mirrors `Settings` in src/lib/backend.ts).
/// Secrets (the DeepL key) are NOT here; they live in the OS keyring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub anki_deck: String,
    /// Extra tags on every mined card (the app always adds "kotoba" and the show's tag).
    pub anki_tags: Vec<String>,
    /// Use DeepL for lines without English subtitles (only when a key is stored).
    pub use_deepl: bool,
    /// Use a local Ollama model when DeepL isn't available.
    pub use_ollama: bool,
    /// Preferred Ollama model; the first installed one when unset or missing.
    pub ollama_model: Option<String>,
    /// English line: "blur" (until hovered), "always", "hidden".
    pub en_mode: String,
    /// Pause while the mouse is over the subtitle line.
    pub pause_on_hover: bool,
    /// Subtitle size multiplier (0.7–1.6).
    pub sub_scale: f64,
    /// Z/X step in ms.
    pub offset_step_ms: u32,
    /// Accent colour preset: "sakura", "twilight", "matcha", "sunset".
    pub accent: String,
    /// Sync subtitle files (next to the video or dropped in) to the voices automatically, once per file.
    pub auto_sync_audio: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            anki_deck: DEFAULT_DECK.to_string(),
            anki_tags: Vec::new(),
            use_deepl: true,
            use_ollama: true,
            ollama_model: None,
            en_mode: "blur".into(),
            pause_on_hover: true,
            sub_scale: 1.0,
            offset_step_ms: 100,
            accent: "sakura".into(),
            auto_sync_audio: true,
        }
    }
}

impl Settings {
    /// Clamp/repair values that came from the UI or an old file.
    pub fn sanitized(mut self) -> Settings {
        let d = Settings::default();
        if self.anki_deck.trim().is_empty() {
            self.anki_deck = d.anki_deck;
        }
        self.anki_tags
            .retain(|t| !t.trim().is_empty() && !t.contains(char::is_whitespace));
        if !matches!(self.en_mode.as_str(), "blur" | "always" | "hidden") {
            self.en_mode = d.en_mode;
        }
        if !self.sub_scale.is_finite() {
            self.sub_scale = 1.0;
        }
        self.sub_scale = self.sub_scale.clamp(0.7, 1.6);
        self.offset_step_ms = self.offset_step_ms.clamp(10, 2000);
        if !matches!(
            self.accent.as_str(),
            "sakura" | "twilight" | "matcha" | "sunset"
        ) {
            self.accent = d.accent;
        }
        self
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

    #[test]
    fn sanitizes_bad_values() {
        let s = Settings {
            anki_deck: " ".into(),
            anki_tags: vec!["ok".into(), "has space".into(), "".into()],
            en_mode: "weird".into(),
            sub_scale: 9.0,
            offset_step_ms: 0,
            accent: "neon".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.anki_deck, DEFAULT_DECK);
        assert_eq!(s.anki_tags, ["ok"]);
        assert_eq!(
            (s.en_mode.as_str(), s.sub_scale, s.offset_step_ms),
            ("blur", 1.6, 10)
        );
        assert_eq!(s.accent, "sakura");
    }
}
