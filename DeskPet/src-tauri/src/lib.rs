pub mod core;
pub mod commands;
pub mod services;
pub mod data;

use std::sync::Mutex;
use tauri::Manager;
use commands::chat_commands::AppState;
use core::prompt::prompt_builder::AnsweringMode;
use services::timeline_service::TimelineState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState {
        ghost: Mutex::new(None),
        db: Mutex::new(None),
        ai_config: Mutex::new(None),
        timeline: Mutex::new(TimelineState::default()),
        screenshot_data: Mutex::new(None),
        answering_mode: Mutex::new(AnsweringMode::Companion),
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
            commands::ghost_commands::auto_save_ghost,
            commands::ghost_commands::find_last_ghost,
            commands::ghost_commands::check_duplicate_signature,
            commands::ghost_commands::curiosity_research,
            commands::chat_commands::chat_with_pet,
            commands::chat_commands::configure_ai,
            commands::chat_commands::init_database,
            commands::chat_commands::ensure_database,
            commands::chat_commands::timeline_tick,
            commands::chat_commands::get_inactivity_status,
            commands::chat_commands::get_emotion_history,
            commands::chat_commands::record_interaction,
            commands::chat_commands::save_chat_message,
            commands::chat_commands::load_chat_history,
            commands::chat_commands::clear_chat_history,
            commands::chat_commands::speak_edge_tts,
            commands::chat_commands::generate_image,
            commands::chat_commands::set_answering_mode,
            commands::chat_commands::get_answering_mode,
            commands::screenshot_commands::capture_screenshot,
            commands::screenshot_commands::capture_region,
            commands::screenshot_commands::analyze_screenshot,
            commands::screenshot_commands::store_screenshot_data,
            commands::screenshot_commands::get_screenshot_data,
            commands::screenshot_commands::close_screenshot_window,
            commands::screenshot_commands::curiosity_background_analyze,
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