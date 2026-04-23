pub mod core;
pub mod commands;
pub mod services;
pub mod data;

use std::sync::Mutex;
use tauri::Manager;
use commands::chat_commands::AppState;
use services::timeline_service::TimelineState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState {
        ghost: Mutex::new(None),
        db: Mutex::new(None),
        ai_config: Mutex::new(None),
        timeline: Mutex::new(TimelineState::default()),
        screenshot_data: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::ghost_commands::generate_ghost,
            commands::ghost_commands::get_ghost_status,
            commands::ghost_commands::apply_event,
            commands::ghost_commands::tick_ghost,
            commands::ghost_commands::save_ghost,
            commands::ghost_commands::load_ghost,
            commands::ghost_commands::transfer_ghost,
            commands::ghost_commands::set_curiosity_level,
            commands::ghost_commands::trigger_curiosity,
            commands::chat_commands::chat_with_pet,
            commands::chat_commands::configure_ai,
            commands::chat_commands::init_database,
            commands::chat_commands::timeline_tick,
            commands::chat_commands::get_inactivity_status,
            commands::chat_commands::get_emotion_history,
            commands::chat_commands::record_interaction,
            commands::screenshot_commands::capture_screenshot,
            commands::screenshot_commands::capture_region,
            commands::screenshot_commands::analyze_screenshot,
            commands::screenshot_commands::store_screenshot_data,
            commands::screenshot_commands::get_screenshot_data,
            commands::screenshot_commands::close_screenshot_window,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            services::hotkey_service::try_register_hotkey(&handle)?;

            for window in app.webview_windows() {
                let _ = window.1.set_shadow(false);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}