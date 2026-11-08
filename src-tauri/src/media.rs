//! Media probing and subtitle extraction with the bundled ffprobe/ffmpeg (tracks, external subtitle files, cache paths).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A track as the UI sees it (mirrors `Track` in src/lib/backend.ts).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// mpv's per-kind track id (1-based; externals continue after embedded subs).
    pub id: u32,
    pub kind: TrackKind,
    pub codec: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_default: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// ffprobe stream index, for extraction (embedded tracks only).
    #[serde(skip)]
    pub stream_index: Option<u32>,
}

/// Track kind as serialized for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

/// Result of probing a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub duration_ms: Option<u64>,
    pub tracks: Vec<Track>,
}

/// Errors from probing/extraction, shown to the user as text.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("Can't read this file: {0}")]
    Probe(String),
    #[error("Couldn't read the subtitles: {0}")]
    Extract(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Deserialize)]
struct FfprobeOut {
    #[serde(default)]
    streams: Vec<FfStream>,
    format: Option<FfFormat>,
}

#[derive(Deserialize)]
struct FfStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
    disposition: Option<std::collections::HashMap<String, i64>>,
}

#[derive(Deserialize)]
struct FfFormat {
    duration: Option<String>,
}

/// Text subtitle codecs we can read; image-based ones (PGS, VobSub) can't be clicked.
const TEXT_SUB_CODECS: &[&str] = &["ass", "ssa", "subrip", "srt", "webvtt", "mov_text", "text"];

/// Subtitle file extensions looked for next to the video.
const SUB_EXTS: &[&str] = &["ass", "ssa", "srt", "vtt"];

/// Parse `ffprobe -print_format json -show_format -show_streams` output into tracks.
pub fn parse_ffprobe(json: &str) -> Result<Probe, MediaError> {
    let out: FfprobeOut =
        serde_json::from_str(json).map_err(|e| MediaError::Probe(e.to_string()))?;
    let (mut v, mut a, mut s) = (0u32, 0u32, 0u32);
    let mut tracks = Vec::new();
    for st in out.streams {
        let kind = match st.codec_type.as_deref() {
            Some("video") => {
                // Cover art inside MKV/MP4 is a "video" stream mpv also counts; keep it, flag via codec.
                v += 1;
                (TrackKind::Video, v)
            }
            Some("audio") => {
                a += 1;
                (TrackKind::Audio, a)
            }
            Some("subtitle") => {
                s += 1;
                (TrackKind::Sub, s)
            }
            _ => continue,
        };
        let tag = |k: &str| {
            st.tags
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty() && v != "und")
        };
        tracks.push(Track {
            id: kind.1,
            kind: kind.0,
            codec: st.codec_name.unwrap_or_else(|| "unknown".into()),
            lang: tag("language"),
            title: tag("title"),
            is_default: st
                .disposition
                .as_ref()
                .and_then(|d| d.get("default"))
                .copied()
                == Some(1),
            external: false,
            path: None,
            stream_index: Some(st.index),
        });
    }
    let duration_ms = out
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| (d * 1000.0).round() as u64);
    Ok(Probe {
        duration_ms,
        tracks,
    })
}

/// Language guessed from a subtitle file name like `Ep01.ja.ass` or `Ep01.English.srt`.
pub fn lang_from_filename(stem_suffix: &str) -> Option<String> {
    let s = stem_suffix.to_ascii_lowercase();
    let parts: Vec<&str> = s.split(['.', '_', '-', ' ', '[', ']']).collect();
    for p in parts.iter().rev() {
        match *p {
            "ja" | "jp" | "jpn" | "japanese" => return Some("jpn".into()),
            "en" | "eng" | "english" => return Some("eng".into()),
            _ => {}
        }
    }
    None
}

/// Subtitle files next to `video` whose name starts with the video's file stem, as external tracks
/// numbered after `first_id`. Only reads the directory; never writes there.
pub fn external_subs(video: &Path, first_id: u32) -> Vec<Track> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str()))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let ext_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| SUB_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)));
            let name_ok = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(stem));
            ext_ok && name_ok && p.is_file()
        })
        .collect();
    found.sort();
    found
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut t = external_track(&p, first_id + i as u32);
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // The language tag sits between the video's stem and the extension (Ep01.ja.ass).
            t.lang = lang_from_filename(name.strip_prefix(stem).unwrap_or(""));
            t
        })
        .collect()
}

/// A subtitle file outside the video (next to it, or added by the user) as a track with `id`.
pub fn external_track(p: &Path, id: u32) -> Track {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = p.file_stem().and_then(|n| n.to_str()).unwrap_or("");
    Track {
        id,
        kind: TrackKind::Sub,
        codec: match ext.as_str() {
            "srt" => "subrip".into(),
            "vtt" => "webvtt".into(),
            other => other.into(),
        },
        lang: lang_from_filename(stem),
        title: Some(name.to_string()),
        is_default: false,
        external: true,
        path: Some(p.to_string_lossy().into_owned()),
        stream_index: None,
    }
}

/// Locations of the bundled tools (`lib/` next to the exe in dev, the resource dir when installed).
#[derive(Debug, Clone)]
pub struct Tools {
    pub ffprobe: PathBuf,
    pub ffmpeg: PathBuf,
}

impl Tools {
    /// Find `ffprobe.exe`/`ffmpeg.exe` in the first of `dirs` that has both.
    pub fn find(dirs: &[PathBuf]) -> Option<Tools> {
        let exe = |n: &str| {
            if cfg!(windows) {
                format!("{n}.exe")
            } else {
                n.to_string()
            }
        };
        dirs.iter().find_map(|d| {
            let (p, m) = (d.join(exe("ffprobe")), d.join(exe("ffmpeg")));
            (p.is_file() && m.is_file()).then_some(Tools {
                ffprobe: p,
                ffmpeg: m,
            })
        })
    }
}

/// A `Command` that never flashes a console window on Windows.
fn quiet(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Probe a media file: embedded tracks plus external subtitle files next to it.
pub fn probe(tools: &Tools, video: &Path) -> Result<Probe, MediaError> {
    if !video.is_file() {
        return Err(MediaError::Probe(format!(
            "{} was not found",
            video.display()
        )));
    }
    let out = quiet(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(video)
        .output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(MediaError::Probe(
            err.lines().last().unwrap_or("ffprobe failed").to_string(),
        ));
    }
    let mut p = parse_ffprobe(&String::from_utf8_lossy(&out.stdout))?;
    if !p
        .tracks
        .iter()
        .any(|t| t.kind == TrackKind::Video || t.kind == TrackKind::Audio)
    {
        return Err(MediaError::Probe("no video or audio streams".into()));
    }
    let next_sub = p.tracks.iter().filter(|t| t.kind == TrackKind::Sub).count() as u32 + 1;
    p.tracks.extend(external_subs(video, next_sub));
    Ok(p)
}

/// Whether a subtitle track holds text we can parse.
pub fn is_text_sub(t: &Track) -> bool {
    t.kind == TrackKind::Sub && TEXT_SUB_CODECS.contains(&t.codec.as_str())
}

/// File extension used when extracting a subtitle codec.
fn extract_ext(codec: &str) -> &'static str {
    match codec {
        "ass" | "ssa" => "ass",
        "webvtt" => "vtt",
        _ => "srt",
    }
}

/// Stable 64-bit FNV-1a hash (cache keys must not change between Rust versions).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Cache key for a video: path + size + modified time, so a replaced file is re-extracted.
pub fn file_key(video: &Path) -> String {
    let meta = std::fs::metadata(video).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{:016x}",
        fnv1a(format!("{}|{size}|{mtime}", video.display()).as_bytes())
    )
}

/// Extract every embedded text subtitle track of `video` into `cache_dir` in ONE ffmpeg run (the
/// file is read once), skipping tracks already extracted. Returns (track id, file) pairs.
pub fn extract_subs(
    tools: &Tools,
    video: &Path,
    tracks: &[Track],
    cache_dir: &Path,
) -> Result<Vec<(u32, PathBuf)>, MediaError> {
    std::fs::create_dir_all(cache_dir)?;
    let key = file_key(video);
    let mut result = Vec::new();
    let mut cmd = quiet(&tools.ffmpeg);
    cmd.args(["-v", "error", "-nostdin", "-y", "-i"]).arg(video);
    let mut pending = 0;
    for t in tracks.iter().filter(|t| is_text_sub(t) && !t.external) {
        let Some(idx) = t.stream_index else { continue };
        let ext = extract_ext(&t.codec);
        let out = cache_dir.join(format!("{key}-s{}.{ext}", t.id));
        result.push((t.id, out.clone()));
        if out.is_file() {
            continue;
        }
        let codec = if t.codec == "mov_text" || t.codec == "text" {
            "srt"
        } else {
            "copy"
        };
        // Write to a temp name and rename after success, so a crash never leaves a half file in the cache.
        let tmp = cache_dir.join(format!("{key}-s{}.part.{ext}", t.id));
        cmd.args(["-map", &format!("0:{idx}"), "-c:s", codec])
            .arg(&tmp);
        pending += 1;
    }
    if pending > 0 {
        let out = cmd.output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(MediaError::Extract(
                err.lines().last().unwrap_or("ffmpeg failed").to_string(),
            ));
        }
        for (id, final_path) in &result {
            let ext = final_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("srt");
            let tmp = cache_dir.join(format!("{key}-s{id}.part.{ext}"));
            if tmp.is_file() {
                std::fs::rename(&tmp, final_path)?;
            }
        }
    }
    Ok(result)
}

/// Grab one frame at `at_ms` as a small JPEG (for "Continue watching" cards).
pub fn thumbnail(tools: &Tools, video: &Path, at_ms: u64, out: &Path) -> Result<(), MediaError> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = out.with_extension("part.jpg");
    let st = quiet(&tools.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-ss",
            &format!("{:.3}", at_ms as f64 / 1000.0),
            "-i",
        ])
        .arg(video)
        .args(["-frames:v", "1", "-vf", "scale=480:-2", "-q:v", "4"])
        .arg(&tmp)
        .status()?;
    if !st.success() || !tmp.is_file() {
        return Err(MediaError::Extract("thumbnail failed".into()));
    }
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// Run ffmpeg writing to a temp file next to `out`, then rename (no half files in the cache).
fn ffmpeg_to(tools: &Tools, out: &Path, args: &[String], what: &str) -> Result<(), MediaError> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let tmp = out.with_extension(format!("part.{ext}"));
    let res = quiet(&tools.ffmpeg)
        .args(["-v", "error", "-nostdin", "-y"])
        .args(args)
        .arg(&tmp)
        .output()?;
    if !res.status.success() || !tmp.is_file() {
        let err = String::from_utf8_lossy(&res.stderr);
        let _ = std::fs::remove_file(&tmp);
        return Err(MediaError::Extract(format!(
            "{what}: {}",
            err.lines().last().unwrap_or("ffmpeg failed")
        )));
    }
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// The video frame at `at_ms` (no subtitles), 640 px wide, as WebP — the card's screenshot.
pub fn card_screenshot(
    tools: &Tools,
    video: &Path,
    at_ms: u64,
    out: &Path,
) -> Result<(), MediaError> {
    let args: Vec<String> = vec![
        "-ss".into(),
        format!("{:.3}", at_ms as f64 / 1000.0),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        "scale=640:-2".into(),
        "-c:v".into(),
        "libwebp".into(),
        "-quality".into(),
        "82".into(),
    ];
    ffmpeg_to(tools, out, &args, "screenshot")
}

/// The audio between `from_ms` and `to_ms` of audio stream `audio_index` (0-based among audio
/// streams) as stereo MP3 — the card's sentence audio.
pub fn audio_clip(
    tools: &Tools,
    video: &Path,
    audio_index: u32,
    from_ms: u64,
    to_ms: u64,
    out: &Path,
) -> Result<(), MediaError> {
    let args: Vec<String> = vec![
        "-ss".into(),
        format!("{:.3}", from_ms as f64 / 1000.0),
        "-to".into(),
        format!("{:.3}", to_ms.max(from_ms + 100) as f64 / 1000.0),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-map".into(),
        format!("0:a:{audio_index}"),
        "-vn".into(),
        "-ac".into(),
        "2".into(),
        "-c:a".into(),
        "libmp3lame".into(),
        "-q:a".into(),
        "4".into(),
    ];
    ffmpeg_to(tools, out, &args, "sentence audio")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn parses_real_ffprobe_output_mkv() {
        let p = parse_ffprobe(&fixture("ffprobe-hi10p-ass.json")).unwrap();
        assert_eq!(p.duration_ms, Some(30_030));
        let kinds: Vec<_> = p
            .tracks
            .iter()
            .map(|t| (t.kind, t.id, t.codec.as_str(), t.lang.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (TrackKind::Video, 1, "h264", None),
                (TrackKind::Audio, 1, "flac", Some("jpn")),
                (TrackKind::Sub, 1, "ass", Some("jpn")),
                (TrackKind::Sub, 2, "subrip", Some("eng")),
            ]
        );
        assert!(p.tracks[2].is_default && !p.tracks[3].is_default);
        assert_eq!(p.tracks[2].title.as_deref(), Some("Japanese (ASS)"));
        assert_eq!(p.tracks[3].stream_index, Some(3));
        assert!(is_text_sub(&p.tracks[2]) && is_text_sub(&p.tracks[3]));
    }

    #[test]
    fn parses_real_ffprobe_output_mp4() {
        let p = parse_ffprobe(&fixture("ffprobe-h264-mp4.json")).unwrap();
        assert_eq!(
            p.tracks.iter().filter(|t| t.kind == TrackKind::Sub).count(),
            0
        );
        assert!(p.duration_ms.unwrap() > 29_000);
    }

    #[test]
    fn serializes_like_the_ts_contract() {
        let p = parse_ffprobe(&fixture("ffprobe-hi10p-ass.json")).unwrap();
        let json = serde_json::to_value(&p.tracks[2]).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"id": 1, "kind": "sub", "codec": "ass", "lang": "jpn", "title": "Japanese (ASS)", "isDefault": true})
        );
    }

    #[test]
    fn garbage_probe_output_is_an_error_not_a_panic() {
        for s in ["", "{", "null", "{\"streams\": 5}", "[]"] {
            assert!(parse_ffprobe(s).is_err(), "{s}");
        }
        assert!(parse_ffprobe("{}").unwrap().tracks.is_empty());
    }

    #[test]
    fn languages_from_file_names() {
        assert_eq!(lang_from_filename(".ja"), Some("jpn".into()));
        assert_eq!(lang_from_filename(".[Japanese]"), Some("jpn".into()));
        assert_eq!(lang_from_filename(".English.forced"), Some("eng".into()));
        assert_eq!(lang_from_filename(""), None);
        assert_eq!(lang_from_filename(".jab"), None);
    }

    #[test]
    fn finds_external_subs_without_writing() {
        let dir = std::env::temp_dir().join(format!("kp-ext-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "Ep01.mkv",
            "Ep01.ja.ass",
            "Ep01.en.srt",
            "Ep02.ja.ass",
            "Ep01.txt",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let subs = external_subs(&dir.join("Ep01.mkv"), 3);
        let got: Vec<_> = subs
            .iter()
            .map(|t| (t.id, t.codec.as_str(), t.lang.as_deref(), t.external))
            .collect();
        assert_eq!(
            got,
            vec![
                (3, "subrip", Some("eng"), true),
                (4, "ass", Some("jpn"), true)
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
