#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod platform;
mod providers;
mod services;
mod state;

use tauri::{AppHandle, Emitter, Manager};
use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            app.manage(AppState::new());
            // Start background selection monitor using Tauri's async runtime
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(selection_monitor(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::save_api_key,
            commands::delete_api_key,
            commands::explain_selection,
            commands::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Select2Explain");
}

/// Polls the OS every 600ms for selected text in the frontmost app.
/// When a non-empty selection appears (and differs from the last one),
/// emits a "selection-changed" event to all windows.
async fn selection_monitor(app: AppHandle) {
    let mut last: String = String::new();
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
        let current = platform::get_selected_text().unwrap_or_default();
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() && trimmed != last {
            last = trimmed.clone();
            let _ = app.emit("selection-changed", trimmed);
        }
    }
}
