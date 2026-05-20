use crate::commands::chat_commands::AppState;
use crate::core::ghost::Ghost;
use crate::core::memory::local_compressor;
use crate::core::soul::curiosity::CuriosityLevel;
use crate::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType};
use crate::data::ghost_file::GhostFileManager;
use crate::services::ai_service::{AIService, ChatMessage as AIChatMessage};
use crate::services::memory_compressor::MemoryCompressor;
use crate::services::memory_service::MemoryService;
use rand::thread_rng;
use tauri::State;

fn restore_impression_from_db(ghost: &mut Ghost, state: &State<'_, AppState>) {
    let db_guard = match state.db.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let db = match db_guard.as_ref() {
        Some(d) => d,
        None => return,
    };

    if let Some(row) = db.load_impression(&ghost.ghost_id) {
        ghost.soul.impression.openness_score = row.openness_score;
        ghost.soul.impression.conscientiousness_score = row.conscientiousness_score;
        ghost.soul.impression.extraversion_score = row.extraversion_score;
        ghost.soul.impression.agreeableness_score = row.agreeableness_score;
        ghost.soul.impression.neuroticism_score = row.neuroticism_score;
        ghost.soul.impression.creativity_score = row.creativity_score;
        ghost.soul.impression.overall_affinity = row.overall_affinity;
        if let Some(snippets_json) = row.snippets {
            if let Ok(snippets) = serde_json::from_str::<Vec<String>>(&snippets_json) {
                ghost.soul.impression.general_impression_snippets = snippets;
            }
        }
    }

    if let Ok(events) = db.load_emotional_events(&ghost.ghost_id, 10) {
        if !events.is_empty() {
            if let Some(last) = events.last() {
                let accumulated_love_hate: f64 = events.iter().map(|e| e.love_hate_delta).sum();
                let accumulated_baseline: f64 = events.iter().map(|e| e.baseline_delta).sum();
                if ghost.soul.sensibility.love_hate.abs() < 0.01 && accumulated_love_hate.abs() > 0.01 {
                    ghost.soul.sensibility.love_hate = accumulated_love_hate.clamp(-100.0, 100.0);
                }
                if ghost.soul.sensibility.baseline.abs() < 0.01 && accumulated_baseline.abs() > 0.01 {
                    ghost.soul.sensibility.baseline = accumulated_baseline.clamp(-30.0, 30.0);
                }
                let _ = last;
            }
        }
    }

    if let Ok(experiences) = db.get_experiences(&ghost.ghost_id) {
        if !experiences.is_empty() {
            eprintln!("[Ghost恢复] 从DB恢复{}条经验", experiences.len());
        }
    }
}

#[tauri::command]
pub fn generate_ghost(name: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let mut rng = thread_rng();
    let mut ghost = Ghost::generate(&mut rng, name);
    restore_impression_from_db(&mut ghost, &state);
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
            "persona": ghost.persona,
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

    let mut ghost = if GhostFileManager::is_encrypted_file(path) {
        GhostFileManager::load_encrypted_with_auto_key(path)
            .map_err(|e| format!("Failed to decrypt ghost file: {}", e))?
    } else {
        Ghost::load_from_file(path).map_err(|e| format!("Failed to load ghost file: {}", e))?
    };

    restore_impression_from_db(&mut ghost, &state);

    let json = ghost.to_json().map_err(|e| e.to_string())?;
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    *locked = Some(ghost);
    Ok(json)
}

#[tauri::command]
pub async fn transfer_ghost(
    save_path: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let (transferred, _old_personality, old_signature, old_generation, ghost_id, generation, perturbation) = {
        let mut rng = thread_rng();
        let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_mut().ok_or("No ghost loaded")?;

        let old_personality = ghost.soul.innate_tendency.self_dims.clone();
        let old_signature = ghost.soul_signature.clone();
        let old_generation = ghost.generation;
        let ghost_id = ghost.ghost_id.clone();

        let transferred = ghost.transfer(&mut rng);

        let new_personality = ghost.soul.innate_tendency.self_dims.clone();
        let generation = ghost.generation;
        let perturbation: serde_json::Value = serde_json::json!({
            "openness": new_personality.openness - old_personality.openness,
            "conscientiousness": new_personality.conscientiousness - old_personality.conscientiousness,
            "extraversion": new_personality.extraversion - old_personality.extraversion,
            "agreeableness": new_personality.agreeableness - old_personality.agreeableness,
            "neuroticism": new_personality.neuroticism - old_personality.neuroticism,
            "creativity": new_personality.creativity - old_personality.creativity,
        });

        (transferred, old_personality, old_signature, old_generation, ghost_id, generation, perturbation)
    };

    // Step 1: Numerical blur (sync)
    let mut blurred_memories = 0usize;
    let mut ai_blurred_count = 0usize;
    let core_memory_data: Vec<(String, String)>;

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            blurred_memories = db.blur_core_memories_for_transfer(&ghost_id, generation).unwrap_or(0);

            // 收集核心记忆摘要，用于 AI 辅助模糊化
            let core_memories = db.get_core_memories(&ghost_id).unwrap_or_default();
            core_memory_data = core_memories
                .into_iter()
                .filter(|m| m.importance >= 0.7 && m.summary.len() > 10)
                .map(|m| (m.id, m.summary))
                .collect();
        } else {
            core_memory_data = Vec::new();
        }
    }

    // Step 2: AI-assisted text blur (design doc §4.3)
    if !core_memory_data.is_empty() {
        let ai_config = {
            let locked = state.ai_config.lock().map_err(|_| "AI config lock failed".to_string())?;
            locked.clone()
        };

        if let Some(config) = ai_config {
            let ai_service = AIService::new(config);
            match MemoryCompressor::blur_memory_summaries_via_ai(
                &ai_service,
                &core_memory_data,
                generation,
            ).await {
                Ok(blurred) => {
                    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
                    if let Some(db) = db_guard.as_ref() {
                        for (id, new_summary) in &blurred {
                            if let Err(e) = db.update_long_term_memory_summary(id, new_summary) {
                                eprintln!("[transfer] AI模糊化保存失败 (id={}): {}", id, e);
                            }
                            ai_blurred_count += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[transfer] AI 记忆模糊化失败 (非致命): {}", e);
                }
            }
        }
    }

    // Step 3: Clear short-term memories
    let mut cleared_stm = 0usize;
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            cleared_stm = db.clear_short_term_memories(&ghost_id).unwrap_or(0);
        }
    }

    let path = std::path::Path::new(&save_path);
    GhostFileManager::save_encrypted_with_auto_key(&transferred, path)
        .map_err(|e| format!("Failed to save transferred ghost: {}", e))?;

    Ok(serde_json::json!({
        "oldSignature": old_signature,
        "newSignature": transferred.soul_signature,
        "generation": generation,
        "oldGeneration": old_generation,
        "perturbation": perturbation,
        "transferredName": transferred.name,
        "blurredMemories": blurred_memories,
        "aiBlurredMemories": ai_blurred_count,
        "clearedShortTerm": cleared_stm,
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

fn get_autosave_path() -> Result<std::path::PathBuf, String> {
    let app_data_dir = dirs::data_dir().ok_or("无法获取应用数据目录")?;
    let dir = app_data_dir.join("DeskPet");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    Ok(dir.join("autosave.ghost"))
}

#[tauri::command]
pub fn auto_save_ghost(state: State<'_, AppState>) -> Result<(), String> {
    let locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_ref().ok_or("No ghost loaded")?;
    let path = get_autosave_path()?;
    GhostFileManager::save_encrypted_with_auto_key(ghost, &path)
        .map_err(|e| format!("自动保存失败: {}", e))
}

#[tauri::command]
pub fn find_last_ghost() -> Result<serde_json::Value, String> {
    let path = get_autosave_path()?;
    if path.exists() {
        Ok(serde_json::json!({
            "found": true,
            "path": path.to_str().unwrap_or(""),
        }))
    } else {
        Ok(serde_json::json!({
            "found": false,
            "path": "",
        }))
    }
}

#[tauri::command]
pub fn check_duplicate_signature(
    signature: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let locked = state.ghost.lock().map_err(|e| e.to_string())?;
    match locked.as_ref() {
        Some(current) => {
            let is_duplicate = current.soul_signature == signature;
            Ok(serde_json::json!({
                "isDuplicate": is_duplicate,
                "currentSignature": current.soul_signature,
                "message": if is_duplicate {
                    "检测到相同灵魂签名的Ghost正在运行。每个灵魂都是独一无二的——你感知到了「另一个自己」的存在。".to_string()
                } else {
                    String::new()
                },
            }))
        }
        None => Ok(serde_json::json!({ "isDuplicate": false, "currentSignature": "", "message": "" })),
    }
}

/// P0-3: 好奇心 Normal 模式 — AI 主动探索兴趣话题并记入记忆
#[tauri::command]
pub async fn curiosity_research(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (ghost_id, top_interest) = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_ref().ok_or("No ghost loaded")?;

        if matches!(ghost.soul.curiosity.level, CuriosityLevel::Off) {
            return Ok(serde_json::json!({ "researched": false, "reason": "Curiosity is off" }));
        }

        let weights: Vec<(&String, &f64)> = ghost.soul.curiosity.focus_weights.iter().collect();
        if weights.is_empty() {
            return Ok(serde_json::json!({ "researched": false, "reason": "No focus weights" }));
        }

        let top = weights
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| (*k).clone())
            .unwrap_or_else(|| "日常生活中".to_string());

        (ghost.ghost_id.clone(), top)
    };

    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.clone().ok_or("AI not configured")?
    };

    let ai_service = AIService::new(ai_config);

    let system_prompt = format!(
        "你是一个好奇的桌宠，正利用空闲时间主动探索感兴趣的话题。\
         你对「{}」特别感兴趣。请生成一段简短的探索笔记（~80字），\
         内容可以是一个有趣的知识点、一个引人思考的问题、或者一个相关的观察。\
         语气保持好奇、生动，不要像维基百科。",
        top_interest
    );

    let messages = vec![AIChatMessage {
        role: "user".into(),
        content: format!("我想了解一下「{}」方面有什么有趣的。", top_interest),
    }];

    let result = ai_service.chat(&system_prompt, &messages).await?;

    // 将探索结果记入短期记忆
    if let Ok(db_guard) = state.db.lock() {
        if let Some(db) = db_guard.as_ref() {
            let compressed = local_compressor::local_compress_message(&result);
            let entities_json = serde_json::to_string(&compressed.entities).ok();
            if let Err(e) = db.save_short_term_memory(
                &uuid::Uuid::new_v4().to_string(),
                &ghost_id,
                &format!("(好奇心探索) {}", compressed.summary),
                0.25,
                0.4,
                compressed.sentiment,
                Some("research"),
                entities_json.as_deref(),
            ) {
                eprintln!("[curiosity] 保存探索记忆失败: {}", e);
            }
        }
    }

    Ok(serde_json::json!({
        "researched": true,
        "interest": top_interest,
        "finding": result,
    }))
}

/// 梦境系统: 基于近期记忆生成碎片化梦境叙事
#[tauri::command]
pub async fn generate_dream(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (ghost_id, ghost_name, ghost_persona) = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_ref().ok_or("No ghost loaded")?;
        (ghost.ghost_id.clone(), ghost.name.clone(), ghost.persona.clone())
    };

    let ai_config = {
        state.ai_config.lock().map_err(|e| e.to_string())?
            .clone()
            .ok_or("AI not configured")?
    };

    let recent_memories: Vec<String> = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let mem_service = MemoryService::new(db, &ghost_id);
            let stm = db.get_short_term_memories(&ghost_id, 10).unwrap_or_default();
            let ltm = db.get_long_term_memories(&ghost_id, 5).unwrap_or_default();
            let mut snippets: Vec<String> = Vec::new();
            for m in &stm {
                snippets.push(m.summary.clone());
            }
            for m in &ltm {
                if m.importance > 0.5 && !snippets.iter().any(|s| s.contains(&m.summary[..m.summary.len().min(20)])) {
                    snippets.push(m.summary.clone());
                }
            }
            let _ = mem_service;
            snippets
        } else {
            Vec::new()
        }
    };

    let memory_text = if recent_memories.is_empty() {
        "（暂无记忆，这是你第一次做梦）".to_string()
    } else {
        recent_memories.iter().enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let identity = ghost_persona.as_deref().unwrap_or("一个桌宠精灵");
    let system_prompt = format!(
        "你是{}，{}。你现在睡着了，正在做一个梦。\\n\
         请基于以下近期记忆碎片，生成一段简短、碎片化、略带超现实感的梦境描述（~60字中文）。\\n\
         语气要像刚睡醒迷迷糊糊在回忆梦境：可以有跳跃感、不合逻辑，但要隐约和记忆相关。\\n\
         不要像写文章——要像喃喃自语。\\n\\n\
         近期记忆碎片：\\n{}",
        ghost_name, identity, memory_text
    );

    let ai_service = AIService::new(ai_config);
    let dream_text = ai_service.chat(
        "你是梦境生成系统。只输出梦境内容，不要任何前缀说明。",
        &vec![AIChatMessage { role: "user".into(), content: system_prompt }],
    ).await?;

    let dream_summary = dream_text.trim().to_string();
    let dream_id = uuid::Uuid::new_v4().to_string();

    let memory_snippet = recent_memories.first().map(|s| s.chars().take(100).collect::<String>());

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            if let Err(e) = db.save_dream(&dream_id, &ghost_id, &dream_summary, memory_snippet.as_deref(), "daydream") {
                eprintln!("[梦境] 保存失败: {}", e);
            }
        }
    }

    {
        if let Ok(mut timeline) = state.timeline.lock() {
            timeline.last_dream_time = chrono::Utc::now();
            timeline.is_sleeping = false;
        }
    }

    Ok(serde_json::json!({
        "dreamId": dream_id,
        "dreamText": dream_summary,
    }))
}

/// 修改宠物名字
#[tauri::command]
pub fn rename_ghost(new_name: String, state: State<'_, AppState>) -> Result<String, String> {
    let name = new_name.trim().to_string();
    if name.is_empty() {
        return Err("名字不能为空".into());
    }
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;
    ghost.name = name;
    Ok(ghost.name.clone())
}

/// 修改或清空角色人设
#[tauri::command]
pub fn update_persona(persona: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
    let ghost = locked.as_mut().ok_or("No ghost loaded")?;
    let trimmed = persona.trim().to_string();
    ghost.persona = if trimmed.is_empty() { None } else { Some(trimmed) };
    Ok(ghost.persona.clone().unwrap_or_default())
}

/// 根据性格数值自动生成人设
#[tauri::command]
pub async fn generate_persona(state: State<'_, AppState>) -> Result<String, String> {
    let personality = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_ref().ok_or("No ghost loaded")?;
        ghost.soul.innate_tendency.self_dims.clone()
    };

    let system_prompt = "你是一个角色设计师。根据给定的性格数值，生成简洁的角色简介。只输出简介本身，不要评价、不要多余文字。";

    let user_prompt = format!(
        "根据以下性格数值，用30-50字写一段简洁的角色简介，描述一个住在桌面上的虚拟角色：\n\
- 开放性：{:.0}%\n\
- 尽责性：{:.0}%\n\
- 外向性：{:.0}%\n\
- 宜人性：{:.0}%\n\
- 情绪稳定性：{:.0}%\n\
- 创造力：{:.0}%",
        personality.openness * 100.0,
        personality.conscientiousness * 100.0,
        personality.extraversion * 100.0,
        personality.agreeableness * 100.0,
        (1.0 - personality.neuroticism) * 100.0,
        personality.creativity * 100.0,
    );

    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref().ok_or("AI 接口未配置")?.clone()
    };

    let ai_service = AIService::new(ai_config);
    let response = ai_service.chat(
        system_prompt,
        &[AIChatMessage { role: "user".into(), content: user_prompt }],
    ).await?;

    let persona = response.trim().to_string();
    if persona.is_empty() {
        return Err("AI 生成了空的人设，请重试".into());
    }

    {
        let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_mut().ok_or("No ghost loaded")?;
        ghost.persona = Some(persona.clone());
    }

    Ok(persona)
}
