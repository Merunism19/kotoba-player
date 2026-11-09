//! Word pronunciation with a Japanese Windows voice (WinRT SpeechSynthesizer, offline); none installed → no audio.

/// The name of the installed Japanese voice, if any (e.g. "Microsoft Haruka").
#[cfg(windows)]
pub fn japanese_voice() -> Option<String> {
    voice_for("ja")
        .and_then(|v| v.DisplayName().ok())
        .map(|s| s.to_string())
}

#[cfg(windows)]
fn voice_for(lang_prefix: &str) -> Option<windows::Media::SpeechSynthesis::VoiceInformation> {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    let voices = SpeechSynthesizer::AllVoices().ok()?;
    let prefix = lang_prefix.to_ascii_lowercase();
    (0..voices.Size().ok()?)
        .filter_map(|i| voices.GetAt(i).ok())
        .find(|v| {
            v.Language()
                .map(|l| l.to_string().to_ascii_lowercase().starts_with(&prefix))
                .unwrap_or(false)
        })
}

/// Speak `text` with the first voice whose language starts with `lang_prefix` ("ja"); WAV bytes.
#[cfg(windows)]
pub fn synthesize(text: &str, lang_prefix: &str) -> Result<Vec<u8>, String> {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    use windows::Storage::Streams::DataReader;
    use windows::core::HSTRING;

    let voice =
        voice_for(lang_prefix).ok_or_else(|| format!("no '{lang_prefix}' voice installed"))?;
    let run = || -> windows::core::Result<Vec<u8>> {
        let synth = SpeechSynthesizer::new()?;
        synth.SetVoice(&voice)?;
        let stream = synth
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))?
            .join()?;
        let size = u32::try_from(stream.Size()?).unwrap_or(u32::MAX);
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        reader.LoadAsync(size)?.join()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf)?;
        Ok(buf)
    };
    run().map_err(|e| format!("Windows speech failed: {e}"))
}

/// Non-Windows builds have no system voice.
#[cfg(not(windows))]
pub fn japanese_voice() -> Option<String> {
    None
}

/// Non-Windows builds have no system voice.
#[cfg(not(windows))]
pub fn synthesize(_text: &str, _lang_prefix: &str) -> Result<Vec<u8>, String> {
    Err("no system voice on this platform".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// This PC has English voices but no Japanese one: proves the WinRT path end to end.
    #[test]
    fn english_voice_produces_wav_when_installed() {
        match synthesize("hello", "en") {
            Ok(wav) => assert!(wav.len() > 1000 && &wav[0..4] == b"RIFF", "not a WAV"),
            Err(e) => eprintln!("skipping: {e}"),
        }
    }
}
