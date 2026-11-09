//! Tauri commands behind src/lib/backend.ts: prepare a file, cues, alignment, tokenizing, dictionary lookup, word audio, per-file state, recents.

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
    /// Folders searched for bundled data (ffmpeg, `dict/jmdict.sqlite`).
    lib_dirs: Vec<PathBuf>,
    dict: OnceLock<Result<jpdict::Dict, String>>,
    settings: Mutex<crate::settings::Settings>,
    /// Card drafts waiting for "Send to Anki", by draft id.
    drafts: Mutex<HashMap<String, Draft>>,
    /// AnkiConnect address (loopback; tests point it at a fake).
    anki_base: String,
}

/// A card prepared by `draft_card`: fields as stored in Anki plus the media files to upload.
struct Draft {
    path: String,
    cue_id: u32,
    fields: serde_json::Map<String, serde_json::Value>,
    /// (file name in Anki's media folder, file on disk)
    media: Vec<(String, PathBuf)>,
    tags: Vec<String>,
}

impl Core {
    /// Build the core: find the bundled tools and load the library store.
    pub fn new(tool_dirs: &[PathBuf], data_dir: PathBuf, cache_dir: PathBuf) -> Core {
        let library = stores::load(&data_dir, Store::Library);
        let settings = stores::load(&data_dir, Store::Settings);
        Core {
            settings: Mutex::new(settings),
            drafts: Mutex::new(HashMap::new()),
            anki_base: crate::anki::DEFAULT_BASE.to_string(),
            tools: Tools::find(tool_dirs),
            data_dir,
            cache_dir,
            library: Mutex::new(library),
            sub_files: Mutex::new(HashMap::new()),
            parsed: Mutex::new(HashMap::new()),
            tokenizer: OnceLock::new(),
            lib_dirs: tool_dirs.to_vec(),
            dict: OnceLock::new(),
        }
    }

    /// The dictionary database, opened on first use from `<lib>/dict/jmdict.sqlite`.
    pub fn dict(&self) -> Result<&jpdict::Dict, String> {
        self.dict
            .get_or_init(|| {
                let path = self
                    .lib_dirs
                    .iter()
                    .map(|d| d.join("dict").join("jmdict.sqlite"))
                    .find(|p| p.is_file())
                    .ok_or_else(|| {
                        "The dictionary is missing (run: node scripts/fetch-deps.mjs).".to_string()
                    })?;
                jpdict::Dict::open(&path).map_err(|e| format!("Dictionary failed to open: {e}"))
            })
            .as_ref()
            .map_err(Clone::clone)
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
    mined_cue_ids: Vec<u32>,
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
    let (remembered, offset_ms, resume_ms, mined_cue_ids) = {
        let lib = lock(&core.library);
        lib.files
            .get(path)
            .map(|s| {
                (
                    (s.jp_sub_id, s.en_sub_id),
                    s.offset_ms,
                    s.resume_ms(),
                    s.mined_cue_ids.clone(),
                )
            })
            .unwrap_or(((None, None), 0, None, Vec::new()))
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
        mined_cue_ids,
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

/// Dictionary lookup at code-point `offset` of a subtitle line (longest match first).
#[tauri::command]
pub async fn lookup_at(
    core: tauri::State<'_, Arc<Core>>,
    text: String,
    offset: usize,
) -> Result<Vec<jpdict::LookupResult>, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.dict().map(|d| d.lookup_at(&text, offset)))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// The first known downstep for `term` read as `reading`, from the dictionary.
fn downstep_of(core: &Core, term: &str, reading: &str) -> Option<usize> {
    let dict = core.dict().ok()?;
    dict.lookup_at(term, 0)
        .into_iter()
        .filter(|r| r.len == term.chars().count())
        .flat_map(|r| r.entries)
        .find(|e| e.term == term && e.reading == reading)
        .and_then(|e| e.pitch.first().map(|p| p.downstep as usize))
}

/// Pronunciation of a word (VOICEVOX with its pitch accent, else a Japanese Windows voice), or null.
#[tauri::command]
pub async fn word_audio(
    core: tauri::State<'_, Arc<Core>>,
    term: String,
    reading: String,
) -> Result<Option<crate::audio::WordAudio>, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let downstep = downstep_of(&core, &term, &reading);
        crate::audio::word_audio(&core.cache_dir, &reading, downstep)
    })
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

// ---- Anki mining -------------------------------------------------------------------------------

impl Core {
    fn anki(&self) -> crate::anki::Client {
        crate::anki::Client::new(&self.anki_base, None)
    }
}

/// Is Anki reachable, and which decks exist.
#[tauri::command]
pub async fn anki_status(
    core: tauri::State<'_, Arc<Core>>,
) -> Result<crate::anki::AnkiStatus, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.anki().status())
        .await
        .map_err(|e| format!("internal error: {e}"))
}

/// Mirrors `MineRequest` in src/lib/backend.ts.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MineRequest {
    path: String,
    jp_sub_id: u32,
    en_sub_id: Option<u32>,
    cue_id: u32,
    start: usize,
    len: usize,
    entry: jpdict::DictEntry,
    #[serde(default)]
    senses: Vec<usize>,
    frame_ms: u64,
    offset_ms: i64,
    audio_id: Option<u32>,
}

/// Mirrors `CardDraft` in src/lib/backend.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDraft {
    draft_id: String,
    fields: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    screenshot_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sentence_audio_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    word_audio_url: Option<String>,
    translation_source: &'static str,
    duplicates: Vec<i64>,
    deck: String,
    decks: Vec<String>,
}

fn data_url_of(path: &Path, mime: &str) -> Option<String> {
    std::fs::read(path).ok().map(|b| {
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(b)
        )
    })
}

fn draft(core: &Core, req: MineRequest) -> Result<CardDraft, String> {
    use crate::mining;
    let tools = core.tools()?;
    let video = Path::new(&req.path);
    let jp = core.track(&req.path, req.jp_sub_id)?;
    let cue = jp
        .cues
        .iter()
        .find(|c| c.id == req.cue_id)
        .ok_or_else(|| "That subtitle line is no longer available.".to_string())?;

    // Media (each part degrades to "absent" instead of failing the whole card).
    let key = format!(
        "{:016x}",
        media::fnv1a(format!("{}|{}|{}", req.path, req.cue_id, req.frame_ms).as_bytes())
    );
    let cards = core.cache_dir.join("cards");
    let shot_name = format!("kotoba_{key}.webp");
    let shot = cards.join(&shot_name);
    let shot_ok = shot.is_file()
        || media::card_screenshot(tools, video, req.frame_ms, &shot)
            .map_err(|e| eprintln!("[kotoba] {e}"))
            .is_ok();
    let audio_name = format!("kotoba_{key}_s.mp3");
    let clip = cards.join(&audio_name);
    let from = (cue.start_ms + req.offset_ms - 250).max(0) as u64;
    let to = (cue.end_ms + req.offset_ms + 250).max(0) as u64;
    let audio_index = req.audio_id.unwrap_or(1).saturating_sub(1);
    let clip_ok = clip.is_file()
        || media::audio_clip(tools, video, audio_index, from, to, &clip)
            .map_err(|e| eprintln!("[kotoba] {e}"))
            .is_ok();
    let downstep = req.entry.pitch.first().map(|p| p.downstep as usize);
    let word_audio = crate::audio::word_audio(&core.cache_dir, &req.entry.reading, downstep)
        .unwrap_or_else(|e| {
            eprintln!("[kotoba] word audio: {e}");
            None
        });

    // Sentence furigana from the tokenizer's units.
    let units: Vec<mining::Unit> = core
        .tokenizer()?
        .word_units(&cue.text)
        .into_iter()
        .map(|u| mining::Unit {
            start: u.start,
            len: u.len,
            text: u.text,
            reading: u.reading,
        })
        .collect();

    // Translation: the aligned English subtitle lines (DeepL/Ollama fallbacks come in Phase 4).
    let translation = req
        .en_sub_id
        .and_then(|en_id| core.track(&req.path, en_id).ok())
        .map(|en| {
            let pairs = subtitles::align(std::slice::from_ref(cue), &en.cues);
            pairs
                .into_iter()
                .flat_map(|(_, ids)| ids)
                .filter_map(|id| {
                    en.cues
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.text.replace('\n', " "))
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    let title = lock(&core.library)
        .files
        .get(&req.path)
        .map(|s| s.title.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| library::title_from_path(&req.path));
    let e = &req.entry;
    let mut fields = serde_json::Map::new();
    let mut put = |k: &str, v: String| {
        fields.insert(k.to_string(), serde_json::Value::String(v));
    };
    put("Word", mining::escape(&e.term));
    put("WordFurigana", mining::anki_furigana(&e.furigana));
    put("WordReading", mining::escape(&e.reading));
    put(
        "PitchPattern",
        e.pitch.first().map(mining::pitch_svg).unwrap_or_default(),
    );
    put(
        "PitchClass",
        e.pitch
            .first()
            .map(|p| p.pattern.clone())
            .unwrap_or_default(),
    );
    put("Meaning", mining::meaning_html(e, &req.senses));
    put(
        "Sentence",
        mining::bold_sentence(&cue.text, req.start, req.len),
    );
    put(
        "SentenceFurigana",
        mining::sentence_furigana(&units, req.start, req.len),
    );
    put("SentenceTranslation", mining::escape(&translation));
    put(
        "Screenshot",
        if shot_ok {
            format!("<img src=\"{shot_name}\">")
        } else {
            String::new()
        },
    );
    put(
        "SentenceAudio",
        if clip_ok {
            format!("[sound:{audio_name}]")
        } else {
            String::new()
        },
    );
    let word_audio_name = word_audio.as_ref().map(|w| {
        let ext = w
            .file
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("wav")
            .to_string();
        format!(
            "kotoba_w_{:016x}.{ext}",
            media::fnv1a(w.file.to_string_lossy().as_bytes())
        )
    });
    put(
        "WordAudio",
        word_audio_name
            .as_ref()
            .map(|n| format!("[sound:{n}]"))
            .unwrap_or_default(),
    );
    let start_ms = (cue.start_ms + req.offset_ms).max(0) as u64;
    put(
        "Source",
        mining::escape(&format!("{title} · {}", mining::clock(start_ms))),
    );

    let mut media_files = Vec::new();
    if shot_ok {
        media_files.push((shot_name, shot.clone()));
    }
    if clip_ok {
        media_files.push((audio_name, clip.clone()));
    }
    if let (Some(n), Some(w)) = (&word_audio_name, &word_audio) {
        media_files.push((n.clone(), w.file.clone()));
    }

    let status = core.anki().status();
    let duplicates = if status.connected {
        core.anki()
            .find_duplicates(&mining::escape(&e.term))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let deck = lock(&core.settings).anki_deck.clone();
    let mut tags = vec!["kotoba".to_string()];
    let show = mining::show_tag(&title);
    if !show.is_empty() {
        tags.push(show);
    }
    tags.extend(lock(&core.settings).anki_tags.iter().cloned());

    let draft_id = format!("{key}-{:x}", library::now_ms());
    lock(&core.drafts).insert(
        draft_id.clone(),
        Draft {
            path: req.path.clone(),
            cue_id: req.cue_id,
            fields: fields.clone(),
            media: media_files,
            tags,
        },
    );
    Ok(CardDraft {
        draft_id,
        fields,
        screenshot_url: shot_ok.then(|| data_url_of(&shot, "image/webp")).flatten(),
        sentence_audio_url: clip_ok.then(|| data_url_of(&clip, "audio/mpeg")).flatten(),
        word_audio_url: word_audio.map(|w| w.url),
        translation_source: if translation.is_empty() {
            "none"
        } else {
            "subtitles"
        },
        duplicates,
        deck,
        decks: status.decks,
    })
}

/// Make the card's media and fields for the clicked word (nothing is sent to Anki yet).
#[tauri::command]
pub async fn draft_card(
    core: tauri::State<'_, Arc<Core>>,
    req: MineRequest,
) -> Result<CardDraft, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || draft(&core, req))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// The user's edits in the mining preview.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardEdits {
    sentence_translation: Option<String>,
    deck: Option<String>,
}

fn add(core: &Core, draft_id: &str, edits: CardEdits) -> Result<i64, String> {
    let (path, cue_id, mut fields, media_files, tags) = {
        let drafts = lock(&core.drafts);
        let d = drafts
            .get(draft_id)
            .ok_or_else(|| "This card draft expired; click the word again.".to_string())?;
        (
            d.path.clone(),
            d.cue_id,
            d.fields.clone(),
            d.media.clone(),
            d.tags.clone(),
        )
    };
    if let Some(t) = edits.sentence_translation {
        fields.insert(
            "SentenceTranslation".into(),
            serde_json::Value::String(crate::mining::escape(t.trim())),
        );
    }
    let deck = edits
        .deck
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| lock(&core.settings).anki_deck.clone());
    let client = core.anki();
    client.ensure_model(&crate::mining::note_type())?;
    client.create_deck(&deck)?;
    for (name, file) in &media_files {
        client.store_media(name, file)?;
    }
    let id = client.add_note(&crate::anki::NewNote {
        deck_name: deck.clone(),
        model_name: crate::anki::MODEL_NAME.to_string(),
        fields,
        tags,
    })?;
    lock(&core.drafts).remove(draft_id);
    {
        let mut lib = lock(&core.library);
        let e = lib.entry(&path);
        if !e.mined_cue_ids.contains(&cue_id) {
            e.mined_cue_ids.push(cue_id);
        }
        core.save_library(&lib);
    }
    {
        let mut s = lock(&core.settings);
        if s.anki_deck != deck {
            s.anki_deck = deck;
            if let Err(e) = stores::save(&core.data_dir, Store::Settings, &*s) {
                eprintln!("[kotoba] saving settings failed: {e}");
            }
        }
    }
    Ok(id)
}

/// Send a draft to Anki; returns the new note id.
#[tauri::command]
pub async fn add_card(
    core: tauri::State<'_, Arc<Core>>,
    draft_id: String,
    edits: CardEdits,
) -> Result<i64, String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || add(&core, &draft_id, edits))
        .await
        .map_err(|e| format!("internal error: {e}"))?
}

/// Open notes in Anki's browser.
#[tauri::command]
pub async fn show_in_anki(
    core: tauri::State<'_, Arc<Core>>,
    note_ids: Vec<i64>,
) -> Result<(), String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ids: Vec<String> = note_ids.iter().map(|i| i.to_string()).collect();
        core.anki().browse(&format!("nid:{}", ids.join(",")))
    })
    .await
    .map_err(|e| format!("internal error: {e}"))?
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

    /// The whole mining flow on the real sample against a fake AnkiConnect: real screenshot and
    /// MP3 clip, furigana, then exactly the expected AnkiConnect actions, and the cue is marked mined.
    #[test]
    fn mine_end_to_end_with_fake_anki() {
        use serde_json::json;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let sample = root.join("../samples/hi10p-ass.mkv");
        let tools_dir = root.join("lib");
        if !sample.is_file() || Tools::find(std::slice::from_ref(&tools_dir)).is_none() {
            eprintln!("skipping: samples or bundled tools missing");
            return;
        }
        let (base, seen) = crate::anki::tests::fake(vec![
            ("version", json!(6)),
            ("deckNames", json!(["Default"])),
            ("findNotes", json!([])),
            ("modelNames", json!(["Basic"])),
            ("createModel", json!({})),
            ("createDeck", json!(1)),
            ("storeMediaFile", json!("stored")),
            ("addNote", json!(1_700_000_000_001i64)),
        ]);
        let tmp = std::env::temp_dir().join(format!("kp-mine-{}", std::process::id()));
        let mut core = Core::new(&[tools_dir], tmp.join("data"), tmp.join("cache"));
        core.anki_base = base;
        let path = sample
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        prepare(&core, &path).unwrap();
        let jp = core.track(&path, 1).unwrap();
        let cue = jp
            .cues
            .iter()
            .find(|c| c.text.starts_with("もう諦めない"))
            .unwrap()
            .clone();
        let entry: jpdict::DictEntry = serde_json::from_value(json!({
            "id": 1, "term": "諦める", "reading": "あきらめる",
            "furigana": [{"text": "諦", "ruby": "あきら"}, {"text": "める"}],
            "senses": [{"glosses": ["to give up", "to abandon"], "pos": ["Ichidan verb"], "notes": []},
                       {"glosses": ["to resign oneself (to)"], "pos": [], "notes": []}],
            "pitch": [{"downstep": 4, "pattern": "nakadaka", "morae": ["あ","き","ら","め","る"]}],
            "common": true, "otherForms": []
        }))
        .unwrap();
        let req = MineRequest {
            path: path.clone(),
            jp_sub_id: 1,
            en_sub_id: Some(2),
            cue_id: cue.id,
            start: 2,
            len: 4,
            entry,
            senses: vec![0],
            frame_ms: 2_000,
            offset_ms: 0,
            audio_id: None,
        };
        let d = draft(&core, req).unwrap();
        let f = |k: &str| d.fields[k].as_str().unwrap().to_string();
        assert_eq!(f("Word"), "諦める");
        assert_eq!(f("WordFurigana"), "諦[あきら]める");
        assert_eq!(f("Sentence"), "もう<b>諦めない</b>って決めたんだ。");
        assert!(
            f("SentenceFurigana").starts_with("もう<b> 諦[あきら]めない</b>"),
            "{}",
            f("SentenceFurigana")
        );
        assert_eq!(
            f("SentenceTranslation"),
            "I decided I&#39;m not giving up anymore.".replace("&#39;", "'")
        );
        assert_eq!(f("Meaning"), "<ol><li>to give up; to abandon</li></ol>");
        assert_eq!(f("PitchClass"), "nakadaka");
        assert!(
            f("Screenshot").starts_with("<img src=\"kotoba_")
                && f("SentenceAudio").starts_with("[sound:kotoba_")
        );
        assert!(
            d.screenshot_url
                .as_deref()
                .unwrap()
                .starts_with("data:image/webp;base64,")
        );
        assert!(
            d.sentence_audio_url
                .as_deref()
                .unwrap()
                .starts_with("data:audio/mpeg;base64,")
        );
        assert_eq!(d.translation_source, "subtitles");
        assert_eq!(d.deck, crate::settings::DEFAULT_DECK);

        let id = add(
            &core,
            &d.draft_id,
            CardEdits {
                sentence_translation: Some("Edited <ok>".into()),
                deck: None,
            },
        )
        .unwrap();
        assert_eq!(id, 1_700_000_000_001);
        let reqs = seen.lock().unwrap().clone();
        let actions: Vec<&str> = reqs.iter().map(|r| r["action"].as_str().unwrap()).collect();
        let tail: Vec<&str> = actions
            .iter()
            .copied()
            .skip_while(|a| *a != "modelNames")
            .collect();
        assert_eq!(tail[..3], ["modelNames", "createModel", "createDeck"]);
        assert_eq!(
            tail.iter().filter(|a| **a == "storeMediaFile").count(),
            2,
            "screenshot + sentence audio"
        );
        assert_eq!(*tail.last().unwrap(), "addNote");
        let note = &reqs.last().unwrap()["params"]["note"];
        assert_eq!(note["modelName"], crate::anki::MODEL_NAME);
        assert_eq!(note["fields"]["SentenceTranslation"], "Edited &lt;ok&gt;");
        assert!(
            note["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t == "kotoba")
        );
        for r in &reqs {
            let a = r["action"].as_str().unwrap();
            assert!(
                !a.starts_with("delete") && !a.starts_with("update")
                    || a.starts_with("updateModel"),
                "unexpected {a}"
            );
        }
        // The cue is now marked as mined for this file.
        let again = prepare(&core, &path).unwrap();
        assert_eq!(again.mined_cue_ids, vec![cue.id]);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
