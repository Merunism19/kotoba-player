//! Sentence translation with a local Ollama model (127.0.0.1:11434 only; free and offline).

use serde_json::json;
use std::time::Duration;

/// Ollama's default local address. Never anything but loopback.
pub const DEFAULT_BASE: &str = "http://127.0.0.1:11434";

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(secs)))
        .build()
        .new_agent()
}

/// Installed model names (empty when Ollama isn't running).
pub fn models(base: &str) -> Vec<String> {
    agent(2)
        .get(format!("{base}/api/tags"))
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_json::<serde_json::Value>().ok())
        .and_then(|v| v["models"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| m["name"].as_str().map(str::to_string))
        .collect()
}

/// The instruction given to the model.
pub fn prompt(text: &str) -> String {
    format!(
        "Translate this line of Japanese anime dialogue into natural English. Reply with the English translation only, no notes or quotes.\n\n{text}"
    )
}

/// Translate one line with `model`.
pub fn translate(base: &str, model: &str, text: &str) -> Result<String, String> {
    let v: serde_json::Value = agent(60)
        .post(format!("{base}/api/generate"))
        .send_json(json!({
            "model": model,
            "prompt": prompt(text),
            "stream": false,
            "options": { "temperature": 0.2 }
        }))
        .map_err(|e| format!("Ollama failed: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("Ollama sent an unexpected reply: {e}"))?;
    let out = v["response"].as_str().unwrap_or("").trim();
    // Some models wrap the answer in quotes or add a "Translation:" label.
    let out = out.strip_prefix("Translation:").unwrap_or(out).trim();
    let out = out
        .trim_matches(|c| c == '"' || c == '“' || c == '”')
        .trim();
    if out.is_empty() {
        Err("Ollama returned no translation.".into())
    } else {
        Ok(out.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_models_and_cleans_the_answer() {
        let (base, seen) = crate::anki::tests::fake_http(vec![
            (
                "/api/tags",
                json!({ "models": [{ "name": "qwen3:8b" }, { "name": "llama3.2" }] }),
            ),
            (
                "/api/generate",
                json!({ "response": " Translation: \"I decided I won't give up.\"\n" }),
            ),
        ]);
        assert_eq!(models(&base), ["qwen3:8b", "llama3.2"]);
        assert_eq!(
            translate(&base, "qwen3:8b", "諦めない").unwrap(),
            "I decided I won't give up."
        );
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs[1]["body"]["model"], "qwen3:8b");
        assert_eq!(reqs[1]["body"]["stream"], false);
    }

    #[test]
    fn not_running_means_no_models() {
        assert!(models("http://127.0.0.1:9").is_empty());
    }
}
