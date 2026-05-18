use tauri::Emitter;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn try_register_hotkey(app: &tauri::AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();

    let screenshot_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyX);
    gs.on_shortcut(screenshot_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = app.emit("screenshot-hotkey", ());
        }
    })
    .map_err(|e| format!("Failed to register screenshot hotkey: {}", e))?;

    let chat_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
    gs.on_shortcut(chat_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = app.emit("chat-hotkey", ());
        }
    })
    .map_err(|e| format!("Failed to register chat hotkey: {}", e))?;

    let settings_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
    gs.on_shortcut(settings_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = app.emit("settings-hotkey", ());
        }
    })
    .map_err(|e| format!("Failed to register settings hotkey: {}", e))?;

    let quit_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyQ);
    gs.on_shortcut(quit_shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = app.emit("quit-hotkey", ());
        }
    })
    .map_err(|e| format!("Failed to register quit hotkey: {}", e))?;

    Ok(())
}
