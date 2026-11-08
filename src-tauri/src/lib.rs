//! Kotoba Player: Tauri entry point (plugins, commands, window setup).

/// Paths of files passed on the command line (e.g. "Open with" from Explorer), for the UI to load.
#[tauri::command]
fn startup_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_libmpv::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![startup_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
