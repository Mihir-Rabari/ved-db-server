// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Tauri commands will be added here as we implement features

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to VedDB Compass.", name)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
