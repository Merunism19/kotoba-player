//! The only way the UI drives mpv: fixed hardened init options, an allowlist of commands and properties.
//!
//! Security review 2026-11-08 (HIGH): the libmpv plugin's own `command`/`set_property` accept any mpv
//! command, so script in the WebView could run `run`/`subprocess`/`load-script` and execute programs.
//! The window's capability now grants only `allow-get-property` from the plugin; everything else goes
//! through these commands, which accept only what the player needs.

use serde_json::{Value, json};
use std::path::Path;
use tauri::{AppHandle, Window};
use tauri_plugin_libmpv::MpvExt;

/// Properties the UI observes (name → mpv format).
const OBSERVED: &[(&str, &str)] = &[
    ("pause", "flag"),
    ("time-pos", "double"),
    ("duration", "double"),
    ("volume", "double"),
    ("speed", "double"),
];

/// Properties the UI may set, with the kind of value allowed.
const SETTABLE: &[(&str, Kind)] = &[
    ("pause", Kind::Flag),
    ("volume", Kind::Number),
    ("speed", Kind::Number),
    ("mute", Kind::Flag),
    ("aid", Kind::TrackId),
    ("sid", Kind::TrackId),
    ("secondary-sid", Kind::TrackId),
    ("sub-visibility", Kind::Flag),
];

#[derive(Clone, Copy)]
enum Kind {
    Flag,
    Number,
    TrackId,
}

/// Fixed mpv options: no scripts, no youtube-dl, no config files, no IPC server, no key bindings,
/// no playlist expansion of untrusted references; subtitles drawn by the WebView except signs.
fn initial_options() -> Value {
    json!({
        "vo": "gpu-next",
        "hwdec": "auto-safe",
        "keep-open": "yes",
        "force-window": "yes",
        "config": "no",
        "load-scripts": "no",
        "ytdl": "no",
        "osc": "no",
        "input-ipc-server": "",
        "input-default-bindings": "no",
        "input-vo-keyboard": "no",
        "load-unsafe-playlists": "no",
        "access-references": "no",
        "sub-auto": "no",
        "sub-visibility": "yes",
        "secondary-sub-visibility": "no",
        "slang": "jpn,ja,jp",
        "alang": "jpn,ja,jp",
        "osd-level": "0",
    })
}

/// Start mpv for this window with the fixed options (idempotent: the plugin ignores a second init).
#[tauri::command]
pub fn mpv_start(app: AppHandle, window: Window) -> Result<(), String> {
    let observed: serde_json::Map<String, Value> = OBSERVED
        .iter()
        .map(|(n, f)| (n.to_string(), json!(f)))
        .collect();
    let config: tauri_plugin_libmpv::MpvConfig = serde_json::from_value(json!({
        "initialOptions": initial_options(),
        "observedProperties": observed,
    }))
    .map_err(|e| format!("mpv config: {e}"))?;
    app.mpv()
        .init(config, window.label())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn is_number(v: &Value) -> bool {
    v.as_f64().is_some_and(f64::is_finite)
        || v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(f64::is_finite)
}

fn check_property(name: &str, value: &Value) -> Result<(), String> {
    let kind = SETTABLE
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
        .ok_or_else(|| format!("mpv property not allowed: {name}"))?;
    let ok = match kind {
        Kind::Flag => value.is_boolean() || matches!(value.as_str(), Some("yes" | "no")),
        Kind::Number => is_number(value),
        Kind::TrackId => {
            matches!(value.as_str(), Some("no" | "auto"))
                || value.as_str().is_some_and(|s| s.parse::<u32>().is_ok())
        }
    };
    if ok {
        Ok(())
    } else {
        Err(format!("bad value for {name}: {value}"))
    }
}

/// Per-file `loadfile` options the UI may pass (comma-separated `key=value`, values %len%-escaped).
fn check_load_options(opts: &str, cache_dir: &Path) -> Result<(), String> {
    let mut rest = opts;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=').ok_or("bad loadfile options")?;
        // Values may be %N%-escaped (N = byte length) to allow commas in paths.
        let (value, tail) = if let Some(esc) = after.strip_prefix('%') {
            let (n, v) = esc.split_once('%').ok_or("bad escape")?;
            let n: usize = n.parse().map_err(|_| "bad escape length")?;
            if v.len() < n || !v.is_char_boundary(n) {
                return Err("bad escape length".into());
            }
            (&v[..n], &v[n..])
        } else {
            match after.find(',') {
                Some(i) => (&after[..i], &after[i..]),
                None => (after, ""),
            }
        };
        match key {
            "sid" | "secondary-sid" | "aid" => check_property(key, &json!(value))?,
            "pause" => check_property("pause", &json!(value))?,
            "start" => {
                if !is_number(&json!(value)) {
                    return Err("bad start".into());
                }
            }
            // Only the signs-only ASS files this app wrote into its own cache.
            "sub-files-append" => {
                let p = Path::new(value);
                let inside = p
                    .canonicalize()
                    .ok()
                    .zip(cache_dir.canonicalize().ok())
                    .is_some_and(|(a, c)| a.starts_with(c));
                if !inside || !value.ends_with(".signs.ass") {
                    return Err("subtitle file not allowed".into());
                }
            }
            other => return Err(format!("loadfile option not allowed: {other}")),
        }
        rest = tail.strip_prefix(',').unwrap_or(tail);
    }
    Ok(())
}

/// Media extensions mpv may open.
const MEDIA_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "webm", "avi", "mov", "ts", "m2ts", "wmv", "flv", "mpg", "mpeg", "ogm",
    "ogv",
];

/// Validate one UI command; returns the (possibly normalized) arguments to send.
fn check_command(name: &str, args: &[Value], cache_dir: &Path) -> Result<Vec<Value>, String> {
    match name {
        "loadfile" => {
            let path = args
                .first()
                .and_then(|v| v.as_str())
                .ok_or("loadfile needs a path")?;
            let p = Path::new(path);
            let ext_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| MEDIA_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)));
            if !p.is_absolute() || !p.is_file() || !ext_ok {
                return Err(format!("not a local video file: {path}"));
            }
            if let Some(flag) = args.get(1).and_then(|v| v.as_str())
                && flag != "replace"
            {
                return Err("loadfile flag not allowed".into());
            }
            if let Some(idx) = args.get(2)
                && !(idx.as_str() == Some("-1") || idx.as_i64() == Some(-1))
            {
                return Err("loadfile index not allowed".into());
            }
            if let Some(opts) = args.get(3).and_then(|v| v.as_str()) {
                check_load_options(opts, cache_dir)?;
            }
            if args.len() > 4 {
                return Err("too many loadfile arguments".into());
            }
            Ok(args.to_vec())
        }
        "seek" => {
            if args.is_empty() || args.len() > 2 || !is_number(&args[0]) {
                return Err("bad seek".into());
            }
            if let Some(mode) = args.get(1)
                && !matches!(
                    mode.as_str(),
                    Some(
                        "relative"
                            | "absolute"
                            | "absolute+exact"
                            | "relative+exact"
                            | "absolute-percent"
                            | "exact"
                    )
                )
            {
                return Err("bad seek mode".into());
            }
            Ok(args.to_vec())
        }
        "stop" if args.is_empty() => Ok(Vec::new()),
        "frame-step" | "frame-back-step" if args.is_empty() => Ok(Vec::new()),
        other => Err(format!("mpv command not allowed: {other}")),
    }
}

/// Run an allowlisted mpv command for this window.
#[tauri::command]
pub fn mpv_command(
    app: AppHandle,
    window: Window,
    core: tauri::State<'_, std::sync::Arc<crate::commands::Core>>,
    name: String,
    args: Vec<Value>,
) -> Result<(), String> {
    let args = check_command(&name, &args, core.cache_dir())?;
    app.mpv()
        .command(&name, &args, window.label())
        .map_err(|e| e.to_string())
}

/// Set an allowlisted mpv property for this window.
#[tauri::command]
pub fn mpv_set(app: AppHandle, window: Window, name: String, value: Value) -> Result<(), String> {
    check_property(&name, &value)?;
    app.mpv()
        .set_property(&name, &value, window.label())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_commands_are_refused() {
        let cache = std::env::temp_dir();
        for (name, args) in [
            ("run", vec![json!("cmd"), json!("/c"), json!("calc")]),
            ("subprocess", vec![json!(["calc"])]),
            ("load-script", vec![json!("C:/evil.lua")]),
            ("loadfile", vec![json!("https://example.invalid/x.m3u8")]),
            ("loadfile", vec![json!("C:/Windows/notepad.exe")]),
            ("set", vec![json!("script-opts"), json!("x")]),
            (
                "screenshot-to-file",
                vec![json!("C:/Users/x/Desktop/a.png")],
            ),
            ("seek", vec![json!("1; run calc")]),
            ("stop", vec![json!("keep-playlist")]),
        ] {
            assert!(
                check_command(name, &args, &cache).is_err(),
                "{name} {args:?}"
            );
        }
    }

    #[test]
    fn player_commands_pass() {
        let dir = std::env::temp_dir().join(format!("kp-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("ep01.mkv");
        std::fs::write(&video, b"x").unwrap();
        let signs = dir.join("abc-s1.signs.ass");
        std::fs::write(&signs, b"x").unwrap();
        let sp = signs.to_string_lossy().to_string();
        let opts = format!(
            "sid=no,secondary-sid=no,pause=no,start=12.500,sub-files-append=%{}%{sp}",
            sp.len()
        );
        let vp = video.to_string_lossy().to_string();
        assert!(
            check_command(
                "loadfile",
                &[json!(vp), json!("replace"), json!("-1"), json!(opts)],
                &dir
            )
            .is_ok()
        );
        assert!(check_command("seek", &[json!(5), json!("relative")], &dir).is_ok());
        assert!(check_command("seek", &[json!(12.3), json!("absolute+exact")], &dir).is_ok());
        assert!(check_command("stop", &[], &dir).is_ok());
        // A signs file outside the cache, or another option, is refused.
        let outside = format!("sub-files-append=%{}%{vp}", vp.len());
        assert!(
            check_command(
                "loadfile",
                &[json!(vp), json!("replace"), json!("-1"), json!(outside)],
                &dir
            )
            .is_err()
        );
        assert!(
            check_command(
                "loadfile",
                &[
                    json!(vp),
                    json!("replace"),
                    json!("-1"),
                    json!("script=x.lua")
                ],
                &dir
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn properties() {
        assert!(check_property("pause", &json!(true)).is_ok());
        assert!(check_property("sid", &json!("3")).is_ok());
        assert!(check_property("sid", &json!("no")).is_ok());
        assert!(check_property("volume", &json!(75)).is_ok());
        assert!(check_property("sid", &json!("1,run")).is_err());
        assert!(check_property("script-opts", &json!("a=b")).is_err());
        assert!(check_property("screenshot-template", &json!("x")).is_err());
        assert!(check_property("volume", &json!("loud")).is_err());
    }
}
