pub mod core;
pub mod commands;
pub mod services;
pub mod data;

use std::sync::Mutex;
use std::io::Cursor;
use tauri::{
    Manager,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use commands::chat_commands::AppState;
use core::prompt::prompt_builder::AnsweringMode;
use services::timeline_service::TimelineState;

fn load_tray_icon() -> tauri::image::Image<'static> {
    let png_bytes = include_bytes!("../icons/32x32.png");
    let decoder = png::Decoder::new(Cursor::new(&png_bytes[..]));
    let mut reader = decoder.read_info().expect("Failed to decode tray icon");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("Failed to read tray icon frame");
    buf.truncate(info.buffer_size());
    tauri::image::Image::new_owned(buf, info.width, info.height)
}

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
            commands::ghost_commands::generate_dream,
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
            commands::chat_commands::check_achievements,
            commands::chat_commands::get_achievements,
            commands::chat_commands::generate_diary,
            commands::chat_commands::get_diary_entries,
            commands::screenshot_commands::capture_screenshot,
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

            // 系统托盘
            let toggle_item = MenuItemBuilder::with_id("toggle", "显示/隐藏").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let icon = load_tray_icon();

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("DeskPet")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            if let Some(window) = app.get_webview_window("pet") {
                                match window.is_visible() {
                                    Ok(true) => { let _ = window.hide(); }
                                    _ => { let _ = window.show(); let _ = window.set_focus(); }
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == MouseButton::Left && button_state == MouseButtonState::Up {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("pet") {
                                match window.is_visible() {
                                    Ok(true) => { let _ = window.hide(); }
                                    _ => { let _ = window.show(); let _ = window.set_focus(); }
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}