//! Word audio chain (VOICEVOX if running → Windows Japanese voice), cached on disk and returned as a data: URL.

use crate::media::fnv1a;
use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Mirrors `WordAudio` in src/lib/backend.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordAudio {
    pub url: String,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit: Option<String>,
    /// The cached WAV on disk (for Anki cards); not sent to the UI.
    #[serde(skip)]
    pub file: PathBuf,
}

fn cache_file(cache_dir: &Path, source: &str, kana: &str, downstep: Option<usize>) -> PathBuf {
    let key = fnv1a(format!("{source}|{kana}|{downstep:?}").as_bytes());
    cache_dir.join("audio").join(format!("{key:016x}.wav"))
}

fn data_url(wav: &[u8]) -> String {
    format!(
        "data:audio/wav;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(wav)
    )
}

fn cached_or(
    path: PathBuf,
    make: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    if let Ok(b) = std::fs::read(&path) {
        return Ok(b);
    }
    let wav = make()?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("part");
    if std::fs::write(&tmp, &wav).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
    Ok(wav)
}

/// Pronounce `reading` (kana). `downstep` is the pitch accent when known (VOICEVOX applies it).
/// `Ok(None)` when no source is available.
pub fn word_audio(
    cache_dir: &Path,
    reading: &str,
    downstep: Option<usize>,
) -> Result<Option<WordAudio>, String> {
    let reading = reading.trim();
    if reading.is_empty() {
        return Ok(None);
    }
    if crate::audio_voicevox::available() {
        let file = cache_file(cache_dir, "voicevox", reading, downstep);
        let wav = cached_or(file.clone(), || {
            crate::audio_voicevox::synthesize(reading, downstep)
        })?;
        return Ok(Some(WordAudio {
            url: data_url(&wav),
            source: "voicevox",
            credit: Some(crate::audio_voicevox::CREDIT.to_string()),
            file,
        }));
    }
    if let Some(voice) = crate::audio_windows::japanese_voice() {
        let file = cache_file(cache_dir, &voice, reading, None);
        let wav = cached_or(file.clone(), || {
            crate::audio_windows::synthesize(reading, "ja")
        })?;
        return Ok(Some(WordAudio {
            url: data_url(&wav),
            source: "windows",
            credit: Some(voice),
            file,
        }));
    }
    Ok(None)
}
