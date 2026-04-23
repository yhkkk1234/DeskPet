use crate::commands::chat_commands::AppState;
use crate::core::ghost::Ghost;
use crate::core::soul::curiosity::CuriosityLevel;
use crate::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType};
use crate::data::ghost_file::GhostFileManager;
use rand::thread_rng;
use tauri::State;

#[tauri::command]
pub fn generate_ghost(name: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let mut rng = thread_rng();
    let ghost = Ghost::generate(&mut rng, name);
    let json = ghost.to_json().map_err(|e| e.to_string())?;
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    *locked = Some(ghost);
    Ok(json)
}

#[tauri::command]
pub fn get_ghost_status(state: State<'_, AppState>) -> Result<String, String> {
    let locked = state.ghost.lock().map_err(|e| e.to_string())?;
    match locked.as_ref() {
        Some(ghost) => Ok(serde_json::json!({
            "ghostId": ghost.ghost_id,
            "name": ghost.name,
            "generation": ghost.generation,
            "loveHate": ghost.soul.sensibility.love_hate,
            "baseline": ghost.soul.sensibility.baseline,
            "personality": ghost.soul.innate_tendency.self_dims,
            "movementStyle": ghost.body.movement_style,
            "impression": {
                "overallAffinity": ghost.soul.impression.overall_affinity,
                "snippetCount": ghost.soul.impression.general_impression_snippets.len(),
            },
            "curiosityLevel": format!("{:?}", ghost.soul.curiosity.level),
        })
        .to_string()),
        None => Err("No ghost loaded".into()),
    }
}

#[tauri::command]
pub fn apply_event(
    event_type: String,
    intensity: f64,
    description: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let et = match event_type.as_str() {
        "UserInitiatedChat" => EmotionalEventType::UserInitiatedChat,
        "UserCaredAboutPet" => EmotionalEventType::UserCaredAboutPet,
        "UserPraisedPet" => EmotionalEventType::UserPraisedPet,
        "UserSharedPersonalStory" => EmotionalEventType::UserSharedPersonalStory,
        "UserCelebratedTogether" => EmotionalEventType::UserCelebratedTogether,
        "FirstConversation" => EmotionalEventType::FirstConversation,
        "BirthdayCelebrated" => EmotionalEventType::BirthdayCelebrated,
        "UserIgnoredPet" => EmotionalEventType::UserIgnoredPet,
        "UserGotAngry" => EmotionalEventType::UserGotAngry,
        "UserDismissedPet" => EmotionalEventType::UserDismissedPet,
        "NormalChat" => EmotionalEventType::NormalChat,
        "CuriosityTriggered" => EmotionalEventType::CuriosityTriggered,
        _ => return Err(format!("Unknown event type: {}", event_type)),
    };

    let mut rng = thread_rng();
    let event =
        EmotionalEvent::new_with_description(et, intensity, description.unwrap_or_default());

    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;
    ghost.soul.apply_emotional_event(&event, &mut rng);

    Ok(serde_json::json!({
        "loveHate": ghost.soul.sensibility.love_hate,
        "baseline": ghost.soul.sensibility.baseline,
        "overallAffinity": ghost.soul.impression.overall_affinity,
    })
    .to_string())
}

#[tauri::command]
pub fn tick_ghost(state: State<'_, AppState>) -> Result<String, String> {
    let mut rng = thread_rng();
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;
    ghost.soul.tick(&mut rng);
    Ok(serde_json::json!({
        "loveHate": ghost.soul.sensibility.love_hate,
        "baseline": ghost.soul.sensibility.baseline,
    })
    .to_string())
}

#[tauri::command]
pub fn save_ghost(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_ref().ok_or("No ghost loaded")?;
    let path = std::path::Path::new(&path);
    GhostFileManager::save_encrypted_with_auto_key(ghost, path)
        .map_err(|e| format!("Failed to save ghost: {}", e))
}

#[tauri::command]
pub fn load_ghost(path: String, state: State<'_, AppState>) -> Result<String, String> {
    let path = std::path::Path::new(&path);

    let ghost = if GhostFileManager::is_encrypted_file(path) {
        GhostFileManager::load_encrypted_with_auto_key(path)
            .map_err(|e| format!("Failed to decrypt ghost file: {}", e))?
    } else {
        Ghost::load_from_file(path).map_err(|e| format!("Failed to load ghost file: {}", e))?
    };

    let json = ghost.to_json().map_err(|e| e.to_string())?;
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    *locked = Some(ghost);
    Ok(json)
}

#[tauri::command]
pub fn transfer_ghost(
    save_path: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut rng = thread_rng();
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;

    let old_personality = ghost.soul.innate_tendency.self_dims.clone();
    let old_signature = ghost.soul_signature.clone();
    let old_generation = ghost.generation;

    let transferred = ghost.transfer(&mut rng);

    let new_personality = ghost.soul.innate_tendency.self_dims.clone();
    let perturbation: serde_json::Value = serde_json::json!({
        "openness": new_personality.openness - old_personality.openness,
        "conscientiousness": new_personality.conscientiousness - old_personality.conscientiousness,
        "extraversion": new_personality.extraversion - old_personality.extraversion,
        "agreeableness": new_personality.agreeableness - old_personality.agreeableness,
        "neuroticism": new_personality.neuroticism - old_personality.neuroticism,
        "creativity": new_personality.creativity - old_personality.creativity,
    });

    let path = std::path::Path::new(&save_path);
    GhostFileManager::save_encrypted_with_auto_key(&transferred, path)
        .map_err(|e| format!("Failed to save transferred ghost: {}", e))?;

    Ok(serde_json::json!({
        "oldSignature": old_signature,
        "newSignature": ghost.soul_signature,
        "generation": ghost.generation,
        "oldGeneration": old_generation,
        "perturbation": perturbation,
        "transferredName": transferred.name,
    }))
}

#[tauri::command]
pub fn set_curiosity_level(
    level: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;

    let new_level = match level.as_str() {
        "Off" => CuriosityLevel::Off,
        "Normal" => CuriosityLevel::Normal,
        "Enhanced" => CuriosityLevel::Enhanced,
        _ => {
            return Err(format!(
                "Unknown curiosity level: {}. Use Off/Normal/Enhanced",
                level
            ))
        }
    };

    ghost
        .soul
        .curiosity
        .set_level(new_level, &ghost.soul.innate_tendency.self_dims);
    ghost.soul.curiosity.last_active_time = Some(chrono::Utc::now());

    Ok(serde_json::json!({
        "level": format!("{:?}", ghost.soul.curiosity.level),
        "intensity": ghost.soul.curiosity.intensity,
        "focusWeights": ghost.soul.curiosity.focus_weights,
        "ownerCuriosity": ghost.soul.curiosity.owner_curiosity,
    }))
}

#[tauri::command]
pub fn trigger_curiosity(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut rng = thread_rng();
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;

    let curiosity = &ghost.soul.curiosity;
    if !curiosity.should_initiate_action() {
        return Ok(serde_json::json!({
            "triggered": false,
            "reason": "Curiosity level too low or off",
        }));
    }

    let mut weights: Vec<(String, f64)> = curiosity
        .focus_weights
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let top_interests: Vec<serde_json::Value> = weights
        .iter()
        .take(3)
        .map(|(k, v)| serde_json::json!({"topic": k, "weight": (v * 100.0).round() / 100.0}))
        .collect();

    let event = EmotionalEvent::new_with_description(
        EmotionalEventType::CuriosityTriggered,
        curiosity.intensity * 0.5,
        format!("好奇心触发: 主动探索{}", weights[0].0),
    );
    ghost.soul.apply_emotional_event(&event, &mut rng);
    ghost.soul.curiosity.last_active_time = Some(chrono::Utc::now());

    Ok(serde_json::json!({
        "triggered": true,
        "intensity": ghost.soul.curiosity.intensity,
        "interests": top_interests,
        "loveHate": ghost.soul.sensibility.love_hate,
    }))
}
