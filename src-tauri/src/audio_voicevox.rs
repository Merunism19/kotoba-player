//! Word pronunciation from a locally running VOICEVOX engine (127.0.0.1:50021 only), with the pitch accent applied when known.

use std::time::Duration;

/// VOICEVOX's default local address. Never anything but loopback (DESIGN §8 rule 1).
const BASE: &str = "http://127.0.0.1:50021";

/// Speaker id used for words: 四国めたん (ノーマル). Its terms require crediting the voice.
pub const SPEAKER: u32 = 2;
/// Credit line VOICEVOX's terms ask for.
pub const CREDIT: &str = "VOICEVOX:四国めたん";

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

/// True when the engine answers within a short timeout.
pub fn available() -> bool {
    agent(Duration::from_millis(400))
        .get(format!("{BASE}/version"))
        .call()
        .is_ok()
}

/// Synthesize `kana` (a reading, so the engine can't misread kanji). When `downstep` is given, the
/// single accent phrase gets that accent (VOICEVOX counts accent as the mora of the high-pitch end;
/// heiban = mora count, which is how its query encodes "no drop").
pub fn synthesize(kana: &str, downstep: Option<usize>) -> Result<Vec<u8>, String> {
    synthesize_at(BASE, kana, downstep)
}

fn synthesize_at(base: &str, kana: &str, downstep: Option<usize>) -> Result<Vec<u8>, String> {
    let a = agent(Duration::from_secs(8));
    let mut query: serde_json::Value = a
        .post(format!("{base}/audio_query"))
        .query("text", kana)
        .query("speaker", SPEAKER.to_string())
        .send_empty()
        .map_err(|e| format!("VOICEVOX audio_query failed: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("VOICEVOX audio_query: bad JSON: {e}"))?;
    if let Some(d) = downstep
        && let Some(phrases) = query
            .get_mut("accent_phrases")
            .and_then(|p| p.as_array_mut())
        && phrases.len() == 1
        && let Some(phrase) = phrases[0].as_object_mut()
    {
        let morae = phrase
            .get("moras")
            .and_then(|m| m.as_array())
            .map(|m| m.len())
            .unwrap_or(0);
        let accent = if d == 0 { morae } else { d.min(morae) };
        if accent > 0 {
            phrase.insert("accent".into(), accent.into());
        }
    }
    a.post(format!("{base}/synthesis"))
        .query("speaker", SPEAKER.to_string())
        .send_json(&query)
        .map_err(|e| format!("VOICEVOX synthesis failed: {e}"))?
        .body_mut()
        .with_config()
        .limit(20 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("VOICEVOX synthesis: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A two-request fake of VOICEVOX's HTTP API on a random loopback port; returns what it received.
    fn fake_engine() -> (String, std::thread::JoinHandle<Vec<(String, String)>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        // network-guard: ok the fake engine's address is 127.0.0.1 with a random port
        let base = format!("http://{}", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = l.accept().unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let path = line.split_whitespace().nth(1).unwrap().to_string();
                let resp: Vec<u8> = if path.starts_with("/audio_query") {
                    br#"{"accent_phrases":[{"moras":[{},{},{},{},{}],"accent":1}],"speedScale":1.0}"#.to_vec()
                } else {
                    b"RIFF....WAVEfake".to_vec()
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    resp.len()
                );
                s.write_all(head.as_bytes()).unwrap();
                s.write_all(&resp).unwrap();
                seen.push((path, String::from_utf8_lossy(&body).into_owned()));
            }
            seen
        });
        (base, h)
    }

    #[test]
    fn applies_pitch_accent_and_returns_wav() {
        let (base, h) = fake_engine();
        let wav = synthesize_at(&base, "あきらめる", Some(4)).unwrap();
        assert!(wav.starts_with(b"RIFF"));
        let seen = h.join().unwrap();
        assert!(seen[0].0.starts_with("/audio_query?text=") && seen[0].0.contains("speaker=2"));
        let sent: serde_json::Value = serde_json::from_str(&seen[1].1).unwrap();
        assert_eq!(sent["accent_phrases"][0]["accent"], 4);
    }

    #[test]
    fn heiban_means_accent_at_the_last_mora() {
        let (base, h) = fake_engine();
        synthesize_at(&base, "ゆうやけ", Some(0)).unwrap();
        let sent: serde_json::Value = serde_json::from_str(&h.join().unwrap()[1].1).unwrap();
        assert_eq!(sent["accent_phrases"][0]["accent"], 5);
    }
}
