//! Sentence translation chain for cards without an English subtitle line: DeepL (owner's key) → local Ollama → none.

use crate::settings::Settings;

/// Where a translation came from (mirrors `CardDraft.translationSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Deepl,
    Ollama,
    None,
}

impl Source {
    /// The string the UI expects.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Deepl => "deepl",
            Source::Ollama => "ollama",
            Source::None => "none",
        }
    }
}

/// Translate `text` with the first provider that works. Errors are logged, never shown as a failure:
/// the user can always type the translation in the preview.
pub fn fallback(settings: &Settings, text: &str) -> (String, Source) {
    if text.trim().is_empty() {
        return (String::new(), Source::None);
    }
    if settings.use_deepl
        && let Some(key) = crate::translate_deepl::stored_key()
    {
        match crate::translate_deepl::translate(&key, text) {
            Ok(t) => return (t, Source::Deepl),
            Err(e) => eprintln!("[kotoba] {e}"),
        }
    }
    if settings.use_ollama {
        let base = crate::translate_ollama::DEFAULT_BASE;
        let models = crate::translate_ollama::models(base);
        let model = settings
            .ollama_model
            .clone()
            .filter(|m| models.contains(m))
            .or_else(|| models.first().cloned());
        if let Some(m) = model {
            match crate::translate_ollama::translate(base, &m, text) {
                Ok(t) => return (t, Source::Ollama),
                Err(e) => eprintln!("[kotoba] {e}"),
            }
        }
    }
    (String::new(), Source::None)
}
