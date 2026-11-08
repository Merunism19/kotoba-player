//! Sentence translation with DeepL (the owner's own API key from the OS keyring; Free keys use api-free.deepl.com).
//!
//! Privacy (DESIGN §8 rule 1): only the one sentence being mined is sent, and only when the line
//! has no English subtitle and the owner has entered a key.

use serde_json::json;
use std::time::Duration;

/// Keyring entry holding the DeepL key (never in settings files or logs).
pub const KEYRING_SERVICE: &str = "kotoba-player";
pub const KEYRING_USER: &str = "deepl-api-key";

/// DeepL's API host for this key: Free keys end in ":fx".
pub fn base_for(key: &str) -> &'static str {
    if key.trim().ends_with(":fx") {
        "https://api-free.deepl.com"
    } else {
        "https://api.deepl.com"
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .build()
        .new_agent()
}

/// Translate one Japanese line into English.
pub fn translate(key: &str, text: &str) -> Result<String, String> {
    translate_at(base_for(key), key, text)
}

pub(crate) fn translate_at(base: &str, key: &str, text: &str) -> Result<String, String> {
    let reply: serde_json::Value = agent()
        .post(format!("{base}/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {}", key.trim()))
        .send_json(json!({ "text": [text], "source_lang": "JA", "target_lang": "EN-US" }))
        .map_err(|e| match e {
            ureq::Error::StatusCode(403) => {
                "DeepL refused the key (check it in Settings).".to_string()
            }
            ureq::Error::StatusCode(456) => "DeepL's monthly free quota is used up.".to_string(),
            other => format!("DeepL failed: {other}"),
        })?
        .body_mut()
        .read_json()
        .map_err(|e| format!("DeepL sent an unexpected reply: {e}"))?;
    reply["translations"][0]["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "DeepL returned no translation.".to_string())
}

/// Check a key without translating anything: characters used / limit this month.
pub fn usage(key: &str) -> Result<(u64, u64), String> {
    usage_at(base_for(key), key)
}

pub(crate) fn usage_at(base: &str, key: &str) -> Result<(u64, u64), String> {
    let v: serde_json::Value = agent()
        .get(format!("{base}/v2/usage"))
        .header("Authorization", format!("DeepL-Auth-Key {}", key.trim()))
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(403) => "DeepL refused the key.".to_string(),
            other => format!("DeepL check failed: {other}"),
        })?
        .body_mut()
        .read_json()
        .map_err(|e| format!("DeepL sent an unexpected reply: {e}"))?;
    Ok((
        v["character_count"].as_u64().unwrap_or(0),
        v["character_limit"].as_u64().unwrap_or(0),
    ))
}

/// The stored key, if any.
pub fn stored_key() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()?
        .get_password()
        .ok()
        .filter(|k| !k.trim().is_empty())
}

/// Store (Some) or remove (None) the key in the OS keyring.
pub fn set_key(key: Option<&str>) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
    match key {
        Some(k) => entry
            .set_password(k.trim())
            .map_err(|e| format!("Couldn't save the key: {e}")),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Couldn't remove the key: {e}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn free_keys_use_the_free_host() {
        assert_eq!(base_for("abc:fx"), "https://api-free.deepl.com");
        assert_eq!(base_for("abc"), "https://api.deepl.com");
    }

    #[test]
    fn translates_and_sends_the_key_as_header() {
        let (base, seen) = crate::anki::tests::fake_http(vec![(
            "/v2/translate",
            serde_json::json!({ "translations": [{ "detected_source_language": "JA", "text": "I won't give up." }] }),
        )]);
        assert_eq!(
            translate_at(&base, "k:fx", "諦めない").unwrap(),
            "I won't give up."
        );
        let r = &seen.lock().unwrap()[0];
        assert_eq!(r["headers"]["authorization"], "DeepL-Auth-Key k:fx");
        assert_eq!(r["body"]["text"][0], "諦めない");
        assert_eq!(r["body"]["target_lang"], "EN-US");
        let _: &Value = r;
    }
}
