//! Tauri commands behind src/lib/backend.ts: prepare a file, cues, alignment, tokenizing, per-file state, recents.

use crate::library::{self, Library};
use crate::media::{self, Tools, Track};
use crate::stores::{self, Store};
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Shared state of the Rust core (managed by Tauri as `Arc<Core>`).
pub struct Core {
    tools: Option<Tools>,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    library: Mutex<Library>,
    /// Per prepared file: subtitle track id → readable subtitle file (extracted or external).
    sub_files: Mutex<HashMap<String, HashMap<u32, PathBuf>>>,
    /// Parsed subtitle tracks, by (file, track id).
    parsed: Mutex<HashMap<(String, u32), Arc<subtitles::Track>>>,
    tokenizer: OnceLock<Result<jptext::Tokenizer, String>>,
}

impl Core {
    /// Build the core: find the bundled tools and load the library store.
    pub fn new(tool_dirs: &[PathBuf], data_dir: PathBuf, cache_dir: PathBuf) -> Core {
        let library = stores::load(&data_dir, Store::Library);
        Core {
            tools: Tools::find(tool_dirs),
            data_dir,
            cache_dir,
            library: Mutex::new(library),
            sub_files: Mutex::new(HashMap::new()),
            parsed: Mutex::new(HashMap::new()),
            tokenizer: OnceLock::new(),
        }
    }

    fn tools(&self) -> Result<&Tools, String> {
        self.tools.as_ref().ok_or_else(|| {
            "The bundled ffmpeg/ffprobe are missing (run: node scripts/fetch-deps.mjs).".to_string()
        })
    }

    /// The tokenizer, created on first use (loading the dictionary takes a moment).
    pub fn tokenizer(&self) -> Result<&jptext::Tokenizer, String> {
        self.tokenizer
            .get_or_init(|| {
                jptext::Tokenizer::new().map_err(|e| format!("Tokenizer failed to load: {e}"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn save_library(&self, lib: &Library) {
        if let Err(e) = stores::save(&self.data_dir, Store::Library, lib) {
            eprintln!("[kotoba] saving library failed: {e}");
        }
    }

    /// Parsed subtitle track `sub_id` of a prepared file (cached).
    fn track(&self, path: &str, sub_id: u32) -> Result<Arc<subtitles::Track>, String> {
        let key = (path.to_string(), sub_id);
        if let Some(t) = lock(&self.parsed).get(&key) {
            return Ok(t.clone());
        }
        let file = lock(&self.sub_files)
            .get(path)
            .and_then(|m| m.get(&sub_id).cloned())
            .ok_or_else(|| {
                format!("Subtitle track {sub_id} isn't available (image-based or not prepared).")
            })?;
        let bytes =
            std::fs::read(&file).map_err(|e| format!("Couldn't read {}: {e}", file.display()))?;
        let track = Arc::new(
            subtitles::parse(&bytes, None).map_err(|e| format!("Couldn't parse subtitles: {e}"))?,
        );
        lock(&self.parsed).insert(key, track.clone());
        Ok(track)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Mirrors `PreparedMedia` in src/lib/backend.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMedia {
    path: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    tracks: Vec<Track>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jp_sub_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    en_sub_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signs_ass_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resume_ms: Option<u64>,
    offset_ms: i64,
}

/// Mirrors `RecentItem` in src/lib/backend.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentItem {
    path: String,
    title: String,
    position_ms: u64,
    duration_ms: u64,
    last_opened: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumb_url: Option<String>,
}

/// Share of characters in `text` that are kana or kanji (0..1), ignoring spaces and punctuation.
fn japanese_ratio(text: &str) -> f64 {
    let (mut jp, mut all) = (0usize, 0usize);
    for c in text.chars().filter(|c| c.is_alphanumeric()) {
        all += 1;
        if jptext::is_kana(c) || jptext::is_kanji(c) {
            jp += 1;
        }
    }
    if all == 0 {
        0.0
    } else {
        jp as f64 / all as f64
    }
}

fn lang_is(t: &Track, codes: &[&str], words: &[&str]) -> bool {
    let lang = t.lang.as_deref().unwrap_or("").to_ascii_lowercase();
    let title = t.title.as_deref().unwrap_or("").to_lowercase();
    codes.iter().any(|c| lang == *c) || words.iter().any(|w| title.contains(w))
}

/// Choose the Japanese and English subtitle tracks: remembered choice, else language tags/titles,
/// else the content of untagged tracks (share of Japanese characters).
fn choose_subs(
    core: &Core,
    path: &str,
    tracks: &[Track],
    remembered: (Option<u32>, Option<u32>),
) -> (Option<u32>, Option<u32>) {
    let readable: Vec<&Track> = tracks.iter().filter(|t| media::is_text_sub(t)).collect();
    let has = |id: Option<u32>| id.filter(|i| readable.iter().any(|t| t.id == *i));
    let ratio = |t: &Track| {
        core.track(path, t.id)
            .map(|tr| {
                let text: String = tr.cues.iter().take(60).map(|c| c.text.as_str()).collect();
                japanese_ratio(&text)
            })
            .unwrap_or(0.0)
    };
    let jp = has(remembered.0)
        .or_else(|| {
            readable
                .iter()
                .find(|t| lang_is(t, &["jpn", "ja", "jp"], &["japanese", "日本語"]))
                .map(|t| t.id)
        })
        .or_else(|| {
            readable
                .iter()
                .filter(|t| t.lang.is_none())
                .find(|t| ratio(t) > 0.3)
                .map(|t| t.id)
        });
    let en = has(remembered.1)
        .or_else(|| {
            readable
                .iter()
                .find(|t| lang_is(t, &["eng", "en"], &["english"]))
                .map(|t| t.id)
        })
        .or_else(|| {
            readable
                .iter()
                .filter(|t| t.lang.is_none() && Some(t.id) != jp)
                .find(|t| {
                    ratio(t) < 0.05 && core.track(path, t.id).is_ok_and(|tr| !tr.cues.is_empty())
                })
                .map(|t| t.id)
        });
    (jp, en.filter(|e| Some(*e) != jp))
}

fn prepare(core: &Core, path: &str) -> Result<PreparedMedia, String> {
    let tools = core.tools()?;
    let video = Path::new(path);
    let probe = media::probe(tools, video).map_err(|e| e.to_string())?;
    let sub_cache = core.cache_dir.join("subs");
    let mut files: HashMap<u32, PathBuf> =
        media::extract_subs(tools, video, &probe.tracks, &sub_cache)
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
    for t in probe.tracks.iter().filter(|t| t.external) {
        if let Some(p) = &t.path {
            files.insert(t.id, PathBuf::from(p));
        }
    }
    lock(&core.sub_files).insert(path.to_string(), files);
    lock(&core.parsed).retain(|(p, _), _| p == path); // keep memory to the current file

    let title = library::title_from_path(path);
    let (remembered, offset_ms, resume_ms) = {
        let lib = lock(&core.library);
        lib.files
            .get(path)
            .map(|s| ((s.jp_sub_id, s.en_sub_id), s.offset_ms, s.resume_ms()))
            .unwrap_or(((None, None), 0, None))
    };
    let (jp_sub_id, en_sub_id) = choose_subs(core, path, &probe.tracks, remembered);

    let signs_ass_path = jp_sub_id
        .and_then(|id| core.track(path, id).ok().map(|t| (id, t)))
        .and_then(|(id, t)| t.signs_only_ass().map(|ass| (id, ass)))
        .and_then(|(id, ass)| {
            let out = sub_cache.join(format!("{}-s{id}.signs.ass", media::file_key(video)));
            std::fs::write(&out, ass)
                .ok()
                .map(|_| out.to_string_lossy().into_owned())
        });

    {
        let mut lib = lock(&core.library);
        lib.touch(path, &title);
        if let Some(d) = probe.duration_ms {
            lib.entry(path).duration_ms = d;
        }
        core.save_library(&lib);
    }
    Ok(PreparedMedia {
        path: path.to_string(),
        title,
        duration_ms: probe.duration_ms,
        tracks: probe.tracks,
        jp_sub_id,
        en_sub_id,
        signs_ass_path,
        resume_ms,
        offset_ms,
    })
}

/// Probe a file, extract its subtitles, pick tracks and restore per-file state.
#[tauri::command]
pub async fn prepare_media(
    core: tauri::State<'_, Arc<Core>>,
    path: String,
) -> Result<PreparedMedia, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || prepare(&core, &path))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// All cues of a subtitle track of the prepared file.
#[tauri::command]
pub async fn get_cues(
    core: tauri::State<'_, Arc<Core>>,
    path: String,
    sub_id: u32,
) -> Result<Vec<subtitles::Cue>, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.track(&path, sub_id).map(|t| t.cues.clone()))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// For each Japanese dialogue cue, the overlapping English cue ids.
#[tauri::command]
pub async fn align_cues(
    core: tauri::State<'_, Arc<Core>>,
    path: String,
    jp_sub_id: u32,
    en_sub_id: u32,
) -> Result<HashMap<u32, Vec<u32>>, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let jp = core.track(&path, jp_sub_id)?;
        let en = core.track(&path, en_sub_id)?;
        Ok(subtitles::align(&jp.cues, &en.cues).into_iter().collect())
    })
    .await
    .map_err(|e| format!("internal error: {e}"))?
}

/// Split a subtitle line into word units.
#[tauri::command]
pub async fn tokenize(
    core: tauri::State<'_, Arc<Core>>,
    text: String,
) -> Result<Vec<jptext::WordUnit>, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.tokenizer().map(|t| t.word_units(&text)))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// Present-but-null vs absent: `Some(None)` clears, `None` leaves unchanged.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<u32>>, D::Error> {
    Option::<u32>::deserialize(d).map(Some)
}

/// Fields of `saveFileState`; absent fields are left unchanged.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatePatch {
    offset_ms: Option<i64>,
    #[serde(default, deserialize_with = "present")]
    jp_sub_id: Option<Option<u32>>,
    #[serde(default, deserialize_with = "present")]
    en_sub_id: Option<Option<u32>>,
}

/// Remember the user's choices for this file.
#[tauri::command]
pub fn save_file_state(
    core: tauri::State<'_, Arc<Core>>,
    path: String,
    state: FileStatePatch,
) -> Result<(), String> {
    let mut lib = lock(&core.library);
    let e = lib.entry(&path);
    if let Some(o) = state.offset_ms {
        e.offset_ms = o.clamp(-600_000, 600_000);
    }
    if let Some(j) = state.jp_sub_id {
        e.jp_sub_id = j;
    }
    if let Some(en) = state.en_sub_id {
        e.en_sub_id = en;
    }
    core.save_library(&lib);
    Ok(())
}

fn thumb_path(core: &Core, video: &Path) -> PathBuf {
    core.cache_dir
        .join("thumbs")
        .join(format!("{}.jpg", media::file_key(video)))
}

/// Remember the playback position; refreshes the card thumbnail in the background now and then.
#[tauri::command]
pub fn save_position(
    core: tauri::State<'_, Arc<Core>>,
    path: String,
    position_ms: u64,
    duration_ms: u64,
) -> Result<(), String> {
    let refresh_thumb = {
        let mut lib = lock(&core.library);
        let e = lib.entry(&path);
        e.position_ms = position_ms;
        if duration_ms > 0 {
            e.duration_ms = duration_ms;
        }
        let stale = e
            .thumb_pos_ms
            .is_none_or(|t| t.abs_diff(position_ms) > 30_000);
        let refresh = position_ms >= 10_000 && stale;
        if refresh {
            e.thumb_pos_ms = Some(position_ms);
        }
        core.save_library(&lib);
        refresh
    };
    if refresh_thumb && let Some(tools) = core.tools.clone() {
        let out = thumb_path(&core, Path::new(&path));
        std::thread::spawn(move || {
            if let Err(e) = media::thumbnail(&tools, Path::new(&path), position_ms, &out) {
                eprintln!("[kotoba] thumbnail failed: {e}");
            }
        });
    }
    Ok(())
}

/// "Continue watching": unfinished files, most recent first.
#[tauri::command]
pub fn recent_files(core: tauri::State<'_, Arc<Core>>) -> Vec<RecentItem> {
    let lib = lock(&core.library);
    lib.continue_watching(20)
        .into_iter()
        .filter(|(p, _)| Path::new(p.as_str()).is_file())
        .map(|(p, s)| RecentItem {
            path: p.clone(),
            title: if s.title.is_empty() {
                library::title_from_path(p)
            } else {
                s.title.clone()
            },
            position_ms: s.position_ms,
            duration_ms: s.duration_ms,
            last_opened: library::iso8601(s.last_opened_ms),
            thumb_url: std::fs::read(thumb_path(&core, Path::new(p.as_str())))
                .ok()
                .map(|b| {
                    format!(
                        "data:image/jpeg;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(b)
                    )
                }),
        })
        .collect()
}

/// Paths passed on the command line ("Open with" from Explorer).
#[tauri::command]
pub fn startup_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_ratio_detects_scripts() {
        assert!(japanese_ratio("もう諦めないって決めたんだ。") > 0.9);
        assert!(japanese_ratio("I decided I'm not giving up.") < 0.01);
        assert_eq!(japanese_ratio("…!?"), 0.0);
    }

    #[test]
    fn patch_distinguishes_null_from_absent() {
        let p: FileStatePatch =
            serde_json::from_str(r#"{"jpSubId": null, "offsetMs": 300}"#).unwrap();
        assert_eq!(p.jp_sub_id, Some(None));
        assert_eq!(p.en_sub_id, None);
        assert_eq!(p.offset_ms, Some(300));
        let p: FileStatePatch = serde_json::from_str(r#"{"enSubId": 2}"#).unwrap();
        assert_eq!(p.en_sub_id, Some(Some(2)));
    }

    #[test]
    fn prepare_end_to_end_on_sample() {
        // Needs the bundled tools and the generated samples; skipped when they're absent.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let sample = root.join("../samples/hi10p-ass.mkv");
        let tools_dir = root.join("lib");
        if !sample.is_file() || Tools::find(std::slice::from_ref(&tools_dir)).is_none() {
            eprintln!("skipping: samples or bundled tools missing");
            return;
        }
        let tmp = std::env::temp_dir().join(format!("kp-core-{}", std::process::id()));
        let core = Core::new(&[tools_dir], tmp.join("data"), tmp.join("cache"));
        let path = sample
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let p = prepare(&core, &path).unwrap();
        assert_eq!((p.jp_sub_id, p.en_sub_id), (Some(1), Some(2)));
        assert!(p.signs_ass_path.is_some(), "the sample has a sign line");
        let jp = core.track(&path, 1).unwrap();
        assert!(
            jp.cues
                .iter()
                .any(|c| c.text == "もう諦めないって決めたんだ。")
        );
        let en = core.track(&path, 2).unwrap();
        let pairs = subtitles::align(&jp.cues, &en.cues);
        assert!(pairs.iter().filter(|(_, e)| !e.is_empty()).count() >= 7);
        assert!(tmp.join("data/library.json").is_file());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
