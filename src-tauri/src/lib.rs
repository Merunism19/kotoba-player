//! Kotoba Player: Tauri entry point (plugins, the Rust core's state, commands).

mod anki;
mod audio;
mod audio_voicevox;
mod audio_windows;
mod commands;
mod library;
mod media;
mod mining;
mod settings;
mod stores;
mod sync;
mod translate;
mod translate_deepl;
mod translate_ollama;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_libmpv::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = app.path();
            // Bundled ffmpeg/ffprobe: the installer's resource dir, or lib/ next to the exe in dev.
            let mut tool_dirs = Vec::new();
            if let Ok(r) = paths.resource_dir() {
                tool_dirs.push(r.join("lib"));
            }
            if let Some(dir) = std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|p| p.to_path_buf()))
            {
                tool_dirs.push(dir.join("lib"));
            }
            let core =
                commands::Core::new(&tool_dirs, paths.app_data_dir()?, paths.app_cache_dir()?);
            let core = Arc::new(core);
            // Load the dictionary in the background so the first hover is instant.
            let warm = core.clone();
            std::thread::spawn(move || {
                let _ = warm.tokenizer();
                let _ = warm.dict();
            });
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::prepare_media,
            commands::get_cues,
            commands::align_cues,
            commands::tokenize,
            commands::lookup_at,
            commands::word_audio,
            commands::anki_status,
            commands::draft_card,
            commands::add_card,
            commands::show_in_anki,
            commands::add_subtitle_file,
            commands::sync_subtitles,
            commands::reset_sync,
            commands::get_settings,
            commands::save_settings,
            commands::set_deepl_key,
            commands::save_file_state,
            commands::save_position,
            commands::recent_files,
            commands::startup_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
