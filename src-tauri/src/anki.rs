//! AnkiConnect client (127.0.0.1:8765 only): status, the app's own note type (create/update), duplicate check, media, add note.
//!
//! Safety rules (DESIGN §8 rule 3): only the "Kotoba Mining" note type is ever created or changed;
//! no other note, deck or note type is edited or deleted. Duplicates are checked before adding.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::time::Duration;

/// AnkiConnect's default local address. Never anything but loopback.
pub const DEFAULT_BASE: &str = "http://127.0.0.1:8765";
/// The note type this app owns.
pub const MODEL_NAME: &str = "Kotoba Mining";
/// Marker in the note type's CSS holding the template version, so updates are detected.
const VERSION_MARK: &str = "/* kotoba-template-version: ";

/// The bundled note type (anki/ folder): fields in order, templates, CSS and version.
#[derive(Debug, Clone)]
pub struct NoteType {
    pub fields: Vec<String>,
    pub front: String,
    pub back: String,
    pub css: String,
    pub version: u32,
}

impl NoteType {
    fn css_with_version(&self) -> String {
        format!("{VERSION_MARK}{} */\n{}", self.version, self.css)
    }
}

/// Talks to AnkiConnect.
pub struct Client {
    base: String,
    key: Option<String>,
    agent: ureq::Agent,
}

#[derive(Deserialize)]
struct Reply {
    result: Value,
    error: Option<String>,
}

/// What the UI shows about Anki.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnkiStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    pub decks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A note to add (fields by name; media must already be stored).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub deck_name: String,
    pub model_name: String,
    pub fields: serde_json::Map<String, Value>,
    pub tags: Vec<String>,
}

impl Client {
    /// A client for `base` (use `DEFAULT_BASE`), with an optional AnkiConnect API key.
    pub fn new(base: &str, key: Option<String>) -> Client {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .new_agent();
        Client {
            base: base.trim_end_matches('/').to_string(),
            key,
            agent,
        }
    }

    fn call<T: DeserializeOwned>(&self, action: &str, params: Value) -> Result<T, String> {
        let mut body = json!({ "action": action, "version": 6, "params": params });
        if let Some(k) = &self.key {
            body["key"] = json!(k);
        }
        let reply: Reply = self
            .agent
            .post(&self.base)
            .send_json(&body)
            .map_err(|e| {
                format!("Anki isn't reachable ({e}). Is Anki open with AnkiConnect installed?")
            })?
            .body_mut()
            .read_json()
            .map_err(|e| format!("AnkiConnect sent an unexpected reply: {e}"))?;
        if let Some(err) = reply.error {
            return Err(format!("AnkiConnect: {err}"));
        }
        serde_json::from_value(reply.result)
            .map_err(|e| format!("AnkiConnect {action}: unexpected result: {e}"))
    }

    /// Connection status and the deck list (never fails; errors go into the status).
    pub fn status(&self) -> AnkiStatus {
        match self.call::<u32>("version", json!({})) {
            Ok(v) => {
                let decks = self
                    .call::<Vec<String>>("deckNames", json!({}))
                    .unwrap_or_default();
                AnkiStatus {
                    connected: true,
                    version: Some(v),
                    decks,
                    error: None,
                }
            }
            Err(e) => AnkiStatus {
                connected: false,
                version: None,
                decks: Vec::new(),
                error: Some(e),
            },
        }
    }

    /// Create the app's note type, or update its templates/CSS when the bundled version is newer.
    /// Returns what was done ("created", "updated", "current").
    pub fn ensure_model(&self, nt: &NoteType) -> Result<&'static str, String> {
        let models: Vec<String> = self.call("modelNames", json!({}))?;
        if !models.iter().any(|m| m == MODEL_NAME) {
            self.call::<Value>(
                "createModel",
                json!({
                    "modelName": MODEL_NAME,
                    "inOrderFields": nt.fields,
                    "css": nt.css_with_version(),
                    "isCloze": false,
                    "cardTemplates": [{ "Name": "Card 1", "Front": nt.front, "Back": nt.back }],
                }),
            )?;
            return Ok("created");
        }
        let fields: Vec<String> =
            self.call("modelFieldNames", json!({ "modelName": MODEL_NAME }))?;
        if let Some(missing) = nt.fields.iter().find(|f| !fields.contains(f)) {
            return Err(format!(
                "The Anki note type \"{MODEL_NAME}\" has no field \"{missing}\". Rename your copy of it in Anki so the app can create a fresh one."
            ));
        }
        let styling: Value = self.call("modelStyling", json!({ "modelName": MODEL_NAME }))?;
        let css = styling.get("css").and_then(|c| c.as_str()).unwrap_or("");
        // No version marker → this "Kotoba Mining" isn't ours (e.g. the user's own type with the
        // same name): never overwrite it (review 2026-11-08).
        let Some(installed) = installed_version(css) else {
            return Err(format!(
                "Anki already has a note type named \"{MODEL_NAME}\" that this app didn't create. Rename it in Anki (Tools → Manage Note Types) so the app can make its own."
            ));
        };
        if installed >= nt.version {
            return Ok("current");
        }
        let templates: Value = self.call("modelTemplates", json!({ "modelName": MODEL_NAME }))?;
        let card_name = templates
            .as_object()
            .and_then(|o| o.keys().next().cloned())
            .unwrap_or_else(|| "Card 1".to_string());
        self.call::<Value>(
            "updateModelTemplates",
            json!({ "model": { "name": MODEL_NAME, "templates": { card_name: { "Front": nt.front, "Back": nt.back } } } }),
        )?;
        self.call::<Value>(
            "updateModelStyling",
            json!({ "model": { "name": MODEL_NAME, "css": nt.css_with_version() } }),
        )?;
        Ok("updated")
    }

    /// Ids of existing notes of our note type whose Word field equals `word` (in any deck).
    pub fn find_duplicates(&self, word: &str) -> Result<Vec<i64>, String> {
        let q = format!("\"note:{MODEL_NAME}\" \"Word:{}\"", escape_query(word));
        self.call("findNotes", json!({ "query": q }))
    }

    /// Copy a local file into Anki's media folder under `filename`; returns the stored name.
    pub fn store_media(&self, filename: &str, path: &std::path::Path) -> Result<String, String> {
        self.call(
            "storeMediaFile",
            json!({ "filename": filename, "path": path.to_string_lossy() }),
        )
    }

    /// Add a note (refuses duplicates in the same deck); returns the note id.
    pub fn add_note(&self, note: &NewNote) -> Result<i64, String> {
        let mut n = serde_json::to_value(note).map_err(|e| e.to_string())?;
        n["options"] = json!({ "allowDuplicate": false, "duplicateScope": "deck" });
        self.call("addNote", json!({ "note": n }))
    }

    /// Create a deck if it doesn't exist yet (AnkiConnect leaves an existing deck untouched).
    pub fn create_deck(&self, deck: &str) -> Result<(), String> {
        self.call::<Value>("createDeck", json!({ "deck": deck }))
            .map(|_| ())
    }

    /// Show notes in Anki's browser.
    pub fn browse(&self, query: &str) -> Result<(), String> {
        self.call::<Value>("guiBrowse", json!({ "query": query }))
            .map(|_| ())
    }
}

/// The version number written into our note type's CSS, if present.
pub fn installed_version(css: &str) -> Option<u32> {
    let rest = &css[css.find(VERSION_MARK)? + VERSION_MARK.len()..];
    rest.split_whitespace().next()?.parse().ok()
}

/// Escape a value for an Anki search query inside double quotes.
pub fn escape_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '"' | '*' | '_' | '\\' | ':' | '(' | ')') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Fake AnkiConnect: answers each action from `answers` (action → result) and records requests.
    pub(crate) fn fake(answers: Vec<(&'static str, Value)>) -> (String, Arc<Mutex<Vec<Value>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        // network-guard: ok fake AnkiConnect on 127.0.0.1 with a random port
        let base = format!("http://{}", l.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let Ok(mut s) = stream else { break };
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
                let req: Value = serde_json::from_slice(&body).unwrap();
                let action = req["action"].as_str().unwrap_or("").to_string();
                seen2.lock().unwrap().push(req);
                let result = answers
                    .iter()
                    .find(|(a, _)| *a == action)
                    .map(|(_, r)| r.clone())
                    .unwrap_or(Value::Null);
                let resp = json!({ "result": result, "error": Value::Null }).to_string();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    resp.len()
                );
                s.write_all(head.as_bytes()).unwrap();
                s.write_all(resp.as_bytes()).unwrap();
            }
        });
        (base, seen)
    }

    /// Generic fake HTTP JSON server on loopback: answers by path prefix, records
    /// `{path, headers (lowercase names), body}` of every request.
    pub(crate) fn fake_http(
        routes: Vec<(&'static str, Value)>,
    ) -> (String, Arc<Mutex<Vec<Value>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        // network-guard: ok fake server on 127.0.0.1 with a random port
        let base = format!("http://{}", l.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let Ok(mut s) = stream else { break };
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_err() {
                    continue;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut headers = serde_json::Map::new();
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        let k = k.trim().to_ascii_lowercase();
                        if k == "content-length" {
                            len = v.trim().parse().unwrap_or(0);
                        }
                        headers.insert(k, Value::String(v.trim().to_string()));
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let answer = routes
                    .iter()
                    .find(|(p, _)| path.starts_with(p))
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| "{}".into());
                seen2
                    .lock()
                    .unwrap()
                    .push(json!({ "path": path, "headers": headers, "body": body }));
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    answer.len()
                );
                s.write_all(head.as_bytes()).unwrap();
                s.write_all(answer.as_bytes()).unwrap();
            }
        });
        (base, seen)
    }

    fn nt(version: u32) -> NoteType {
        NoteType {
            fields: vec!["Word".into(), "Sentence".into()],
            front: "{{Word}}".into(),
            back: "{{Sentence}}".into(),
            css: ".card{}".into(),
            version,
        }
    }

    fn actions(seen: &Arc<Mutex<Vec<Value>>>) -> Vec<String> {
        seen.lock()
            .unwrap()
            .iter()
            .map(|r| r["action"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn creates_model_when_missing() {
        let (base, seen) = fake(vec![
            ("modelNames", json!(["Basic"])),
            ("createModel", json!({})),
        ]);
        let c = Client::new(&base, None);
        assert_eq!(c.ensure_model(&nt(1)).unwrap(), "created");
        assert_eq!(actions(&seen), vec!["modelNames", "createModel"]);
        let req = &seen.lock().unwrap()[1];
        assert_eq!(req["version"], 6);
        assert_eq!(req["params"]["modelName"], MODEL_NAME);
        assert!(
            req["params"]["css"]
                .as_str()
                .unwrap()
                .starts_with("/* kotoba-template-version: 1 */")
        );
    }

    #[test]
    fn updates_only_our_model_when_older() {
        let (base, seen) = fake(vec![
            ("modelNames", json!(["Basic", MODEL_NAME])),
            ("modelFieldNames", json!(["Word", "Sentence"])),
            (
                "modelStyling",
                json!({ "css": "/* kotoba-template-version: 1 */\n.card{}" }),
            ),
            (
                "modelTemplates",
                json!({ "Card 1": { "Front": "", "Back": "" } }),
            ),
        ]);
        let c = Client::new(&base, None);
        assert_eq!(c.ensure_model(&nt(2)).unwrap(), "updated");
        let a = actions(&seen);
        assert_eq!(
            &a[a.len() - 2..],
            ["updateModelTemplates", "updateModelStyling"]
        );
        for r in seen.lock().unwrap().iter() {
            if let Some(m) = r["params"]["model"]["name"].as_str() {
                assert_eq!(m, MODEL_NAME, "never touches other note types");
            }
        }
    }

    #[test]
    fn current_model_is_left_alone() {
        let (base, seen) = fake(vec![
            ("modelNames", json!([MODEL_NAME])),
            ("modelFieldNames", json!(["Word", "Sentence"])),
            (
                "modelStyling",
                json!({ "css": "/* kotoba-template-version: 2 */" }),
            ),
        ]);
        assert_eq!(
            Client::new(&base, None).ensure_model(&nt(2)).unwrap(),
            "current"
        );
        assert!(!actions(&seen).iter().any(|a| a.starts_with("update")));
    }

    #[test]
    fn add_note_refuses_duplicates_and_sends_key() {
        let (base, seen) = fake(vec![("addNote", json!(1_700_000_000_123i64))]);
        let c = Client::new(&base, Some("secret".into()));
        let mut fields = serde_json::Map::new();
        fields.insert("Word".into(), json!("諦める"));
        let id = c
            .add_note(&NewNote {
                deck_name: "Mining".into(),
                model_name: MODEL_NAME.into(),
                fields,
                tags: vec!["kotoba".into()],
            })
            .unwrap();
        assert_eq!(id, 1_700_000_000_123);
        let req = &seen.lock().unwrap()[0];
        assert_eq!(req["key"], "secret");
        assert_eq!(req["params"]["note"]["options"]["allowDuplicate"], false);
        assert_eq!(req["params"]["note"]["deckName"], "Mining");
    }

    #[test]
    fn unreachable_anki_is_a_status_not_a_panic() {
        let s = Client::new("http://127.0.0.1:9", None).status();
        assert!(!s.connected && s.error.is_some());
    }

    #[test]
    fn helpers() {
        assert_eq!(
            installed_version("/* kotoba-template-version: 12 */\n.x{}"),
            Some(12)
        );
        assert_eq!(installed_version(".x{}"), None);
        assert_eq!(escape_query(r#"a"b*c"#), r#"a\"b\*c"#);
    }
}
