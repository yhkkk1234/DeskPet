use std::sync::Mutex;
use tauri::State;

use crate::core::ghost::Ghost;
use crate::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType};
use crate::core::prompt::prompt_builder::PromptBuilder;
use crate::core::prompt::token_budget::TokenBudget;
use crate::data::database::Database;
use crate::services::ai_service::{AIService, AIProviderConfig, ChatMessage};
use crate::services::memory_service::MemoryService;
use crate::core::memory::local_compressor;
use crate::services::sentiment_service::{parse_event_type, SentimentAnalyzer};
use crate::services::memory_compressor::MemoryCompressor;
use crate::services::timeline_service::TimelineState;

pub struct AppState {
    pub ghost: Mutex<Option<Ghost>>,
    pub db: Mutex<Option<Database>>,
    pub ai_config: Mutex<Option<AIProviderConfig>>,
    pub timeline: Mutex<TimelineState>,
    pub screenshot_data: Mutex<Option<String>>,
}

#[tauri::command]
pub async fn chat_with_pet(
    message: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ghost = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().ok_or("没有加载 Ghost，请先生成桌宠灵魂")?.clone()
    };

    let system_prompt = build_system_prompt(&ghost, &state)?;

    let mut messages = Vec::new();

    let is_first_conversation: bool;
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let history = db.load_chat_history(&ghost.ghost_id, 10)?;
            is_first_conversation = history.is_empty();
            for msg in &history {
                if msg.role == "user" || msg.role == "assistant" {
                    messages.push(ChatMessage {
                        role: msg.role.clone(),
                        content: msg.content.clone(),
                    });
                }
            }
        } else {
            is_first_conversation = true;
        }
    }

    messages.push(ChatMessage {
        role: "user".into(),
        content: message.clone(),
    });

    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref()
            .ok_or("AI 接口未配置。请点击右侧面板「配置 AI 接口」按钮，填写 Endpoint、API Key 和 Model 后重试。")?
            .clone()
    };

    let ai_service = AIService::new(ai_config);

    let raw_response = ai_service.chat(&system_prompt, &messages).await?;

    // 检查 AI 是否请求生成图像：格式 [GENERATE_IMAGE: 描述]
    let mut generated_image = None;
    let response = if let Some(pos) = raw_response.find("[GENERATE_IMAGE:") {
        if let Some(end) = raw_response[pos..].find(']') {
            let marker = &raw_response[pos..pos + end + 1];
            if let Some(prompt_start) = marker.find(':') {
                let prompt = marker[prompt_start + 1..marker.len() - 1].trim();
                match ai_service.generate_image(prompt).await {
                    Ok(img_data) => {
                        generated_image = Some(img_data);
                    }
                    Err(e) => {
                        eprintln!("[图像生成] 失败: {}", e);
                    }
                }
                raw_response.replace(marker, "").trim().to_string()
            } else {
                raw_response.clone()
            }
        } else {
            raw_response.clone()
        }
    } else {
        raw_response.clone()
    };

    let event_type_str: String;
    let sentiment_love_hate: f64;
    let sentiment_snippet: Option<String>;
    let impression_deltas: (f64, f64, f64, f64, f64, f64);
    let emotional_event: EmotionalEvent;

    match SentimentAnalyzer::analyze(&ai_service, &message, &response, &ghost.name).await {
        Ok(result) => {
            event_type_str = if is_first_conversation {
                "FirstConversation".to_string()
            } else {
                result.event_type.clone()
            };
            sentiment_love_hate = if is_first_conversation { 2.0 } else { result.love_hate_hint };
            sentiment_snippet = result.impression.snippet.clone();
            impression_deltas = (
                result.impression.openness_delta,
                result.impression.conscientiousness_delta,
                result.impression.extraversion_delta,
                result.impression.agreeableness_delta,
                result.impression.neuroticism_delta,
                result.impression.creativity_delta,
            );

            emotional_event = EmotionalEvent::new_with_description(
                parse_event_type(&event_type_str),
                result.intensity,
                format!("对话: {} -> {}", truncate(&message, 50), truncate(&response, 50)),
            );
        }
        Err(e) => {
            eprintln!("[情感分析] 分析失败（不影响对话）: {}", e);
            event_type_str = "NormalChat".into();
            sentiment_love_hate = 0.0;
            sentiment_snippet = None;
            impression_deltas = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            emotional_event = EmotionalEvent::new(EmotionalEventType::NormalChat, 0.0);
        }
    }

    if let Ok(mut locked) = state.ghost.lock() {
        if let Some(g) = locked.as_mut() {
            let mut rng = rand::thread_rng();

            if sentiment_love_hate != 0.0 {
                g.soul.sensibility.apply_event(sentiment_love_hate);
            }
            if emotional_event.baseline_delta != 0.0 {
                g.soul.sensibility.shift_baseline(emotional_event.baseline_delta);
            }
            g.soul.sensibility.apply_spring(&mut rng);

            let imp_event = crate::core::soul::impression::ImpressionEvent {
                openness_delta: impression_deltas.0,
                conscientiousness_delta: impression_deltas.1,
                extraversion_delta: impression_deltas.2,
                agreeableness_delta: impression_deltas.3,
                neuroticism_delta: impression_deltas.4,
                creativity_delta: impression_deltas.5,
                snippet: sentiment_snippet,
            };
            g.soul.impression.apply_event(&imp_event, &g.soul.innate_tendency.preferred_dims);
        }
    }

    let ghost_snapshot = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().ok_or("No ghost loaded")?.clone()
    };

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let mem_service = MemoryService::new(db, &ghost_snapshot.ghost_id);

            let compressed_user = local_compressor::local_compress_message(&message);
            let user_sentiment = compressed_user.sentiment;
            let user_entities = serde_json::to_string(&compressed_user.entities).ok();

            let _ = mem_service.add_short_term_memory(
                &format!("用户说：{}", compressed_user.summary),
                0.4,
                0.5,
                user_sentiment,
                Some("chat"),
                user_entities.as_deref(),
            );

            let compressed_response = local_compressor::local_compress_message(&response);
            let response_sentiment = compressed_response.sentiment;
            let response_entities = serde_json::to_string(&compressed_response.entities).ok();

            let response_summary = if compressed_response.summary.len() > 200 {
                let ts: String = compressed_response.summary.chars().take(200).collect();
                format!("{}…", ts)
            } else {
                compressed_response.summary.clone()
            };
            let _ = mem_service.add_short_term_memory(
                &format!("你回复：{}", response_summary),
                0.4,
                0.5,
                response_sentiment,
                Some("chat"),
                response_entities.as_deref(),
            );

            // P0-2: 经验使用检测 — 检查AI回复是否涉及已有经验，若涉及则增加熟练度
            if let Ok(experiences) = db.get_experiences(&ghost_snapshot.ghost_id) {
                let response_lower = response.to_lowercase();
                for exp in &experiences {
                    let name_lower = exp.name.to_lowercase();
                    if response_lower.contains(&name_lower) {
                        let new_proficiency = (exp.proficiency + 0.01).min(1.0);
                        let _ = db.update_experience_proficiency(&exp.id, new_proficiency);
                    }
                }
            }
        }
    }

    let should_compress = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let mem_service = MemoryService::new(db, &ghost_snapshot.ghost_id);
            mem_service.should_compress()?
        } else {
            false
        }
    };

    if should_compress {
        let compress_ai_config = {
            let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
            locked.as_ref().cloned()
        };
        if let Some(cfg) = compress_ai_config {
            let ghost_id = ghost_snapshot.ghost_id.clone();
            let ai = AIService::new(cfg);

            let stm_data = {
                let db_guard = state.db.lock().map_err(|e| e.to_string())?;
                match db_guard.as_ref() {
                    Some(db) => MemoryCompressor::fetch_stm_data(db, &ghost_id).ok(),
                    None => None,
                }
            };

            if let Some(stm_data) = stm_data {
                let ltm_items = match MemoryCompressor::compress_via_ai(&ai, &stm_data, &ghost_snapshot.name).await {
                    Ok(items) => items,
                    Err(e) => {
                        eprintln!("[记忆压缩] AI压缩失败（不影响对话）: {}", e);
                        vec![]
                    }
                };

                if !ltm_items.is_empty() {
                    let mut experiences = Vec::new();
                    for item in &ltm_items {
                        if item.importance >= 0.5 {
                            if let Ok(exp) = MemoryCompressor::extract_experience_via_ai(&ai, &item.summary).await {
                                experiences.push(exp);
                            }
                        }
                    }

                    let stm_ids: Vec<String> = stm_data.iter().map(|(id, _, _)| id.clone()).collect();

                    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
                    if let Some(db) = db_guard.as_ref() {
                        match MemoryCompressor::save_compress_results(db, &ghost_id, &ltm_items, &stm_ids, &experiences) {
                            Ok(result) => eprintln!("[记忆压缩] 完成: {}条长期记忆, {}条经验", result.new_ltm_count, result.new_experience_count),
                            Err(e) => eprintln!("[记忆压缩] 保存失败: {}", e),
                        }
                    }
                }
            }
        }
    }

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let event_id = uuid::Uuid::new_v4().to_string();
            let _ = db.save_emotional_event(
                &event_id,
                &ghost_snapshot.ghost_id,
                &event_type_str,
                sentiment_love_hate.abs() / 5.0,
                sentiment_love_hate,
                emotional_event.baseline_delta,
                Some(&truncate(&message, 100)),
            );

            let imp = &ghost_snapshot.soul.impression;
            let snippets_json = serde_json::to_string(&imp.general_impression_snippets).unwrap_or_else(|_| "[]".into());
            let _ = db.save_impression(
                &ghost_snapshot.ghost_id,
                imp.openness_score,
                imp.conscientiousness_score,
                imp.extraversion_score,
                imp.agreeableness_score,
                imp.neuroticism_score,
                imp.creativity_score,
                imp.overall_affinity,
                &snippets_json,
            );

            let chunk_id = uuid::Uuid::new_v4().to_string();
            let chunk_summary = format!("{}: {} → {}", ghost_snapshot.name, truncate(&message, 40), truncate(&response, 40));
            let _ = db.save_conversation_chunk(
                &chunk_id,
                &ghost_snapshot.ghost_id,
                &chunk_summary,
                if sentiment_love_hate.abs() > 3.0 { 0.7 } else { 0.4 },
                None,
            );
        }
    }

    if let Ok(mut timeline) = state.timeline.lock() {
        timeline.record_interaction();
    }

    let mut result = serde_json::json!({
        "response": response,
        "loveHate": ghost_snapshot.soul.sensibility.love_hate,
        "baseline": ghost_snapshot.soul.sensibility.baseline,
        "personality": ghost_snapshot.soul.innate_tendency.self_dims,
        "sentiment": {
            "eventType": event_type_str,
            "loveHateHint": sentiment_love_hate,
        },
        "impression": {
            "overallAffinity": ghost_snapshot.soul.impression.overall_affinity,
            "latestSnippet": ghost_snapshot.soul.impression.get_latest_snippet(),
        },
    });

    // 如果生成了图像，添加到返回结果
    if let Some(ref img) = generated_image {
        result["generatedImage"] = serde_json::Value::String(img.clone());
    }

    Ok(result)
}

#[tauri::command]
pub fn configure_ai(
    endpoint: String,
    api_key: String,
    model: String,
    vision_model: Option<String>,
    image_model: Option<String>,
    image_gen_endpoint: Option<String>,
    image_gen_api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if endpoint.trim().is_empty() {
        return Err("Endpoint 不能为空".into());
    }
    if api_key.trim().is_empty() {
        return Err("API Key 不能为空".into());
    }
    if model.trim().is_empty() {
        return Err("Model 不能为空".into());
    }

    let config = AIProviderConfig {
        endpoint,
        api_key,
        model,
        vision_model,
        image_model,
        image_gen_endpoint: image_gen_endpoint.filter(|s| !s.trim().is_empty()),
        image_gen_api_key: image_gen_api_key.filter(|s| !s.trim().is_empty()),
        is_default: true,
    };

    {
        let mut locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        *locked = Some(config);
    }

    if state.db.lock().map_err(|e| e.to_string())?.is_none() {
        let app_data_dir = dirs::data_dir()
            .ok_or("无法获取应用数据目录")?;
        let db_path = app_data_dir.join("DeskPet").join("deskpet.db");
        std::fs::create_dir_all(db_path.parent().ok_or("无法创建数据库目录")?)
            .map_err(|e| format!("创建目录失败: {}", e))?;
        let db = Database::new(db_path.to_str().ok_or("数据库路径无效")?)?;
        let mut locked = state.db.lock().map_err(|e| e.to_string())?;
        *locked = Some(db);
    }

    Ok(())
}

#[tauri::command]
pub fn init_database(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = Database::new(&path)?;
    let mut locked = state.db.lock().map_err(|e| e.to_string())?;
    *locked = Some(db);
    Ok(())
}

#[tauri::command]
pub fn timeline_tick(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut rng = rand::thread_rng();
    let mut timeline = state.timeline.lock().map_err(|e| e.to_string())?;
    let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;

    match locked.as_mut() {
        Some(ghost) => {
            let result = crate::services::timeline_service::TimelineService::tick(
                ghost, &mut timeline, &mut rng,
            );

            let mut decayed_experiences = 0usize;
            if let Some(db) = state.db.lock().map_err(|e| e.to_string())?.as_ref() {
                let mem_service = MemoryService::new(db, &ghost.ghost_id);
                decayed_experiences = mem_service.decay_experience_proficiency().unwrap_or(0);

                // P1-5: 不活跃事件持久化到DB
                if let Some(ref inactivity_evt) = result.inactivity_event {
                    let description = format!("主人已经{}小时没有互动了...", inactivity_evt.hours_away);
                    let _ = db.save_emotional_event(
                        &uuid::Uuid::new_v4().to_string(),
                        &ghost.ghost_id,
                        &inactivity_evt.event_type,
                        inactivity_evt.intensity,
                        inactivity_evt.love_hate_delta,
                        0.0,
                        Some(&description),
                    );
                }
            }
            if decayed_experiences > 0 {
                eprintln!("[经验衰减] {}条经验熟练度已衰减", decayed_experiences);
            }

            Ok(serde_json::json!({
                "loveHate": result.love_hate_after_tick,
                "baseline": result.baseline_after_tick,
                "inactivityEvent": result.inactivity_event,
                "curiosityTriggered": result.curiosity_triggered,
            }))
        }
        None => Err("No ghost loaded".into()),
    }
}

#[tauri::command]
pub fn get_inactivity_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let timeline = state.timeline.lock().map_err(|e| e.to_string())?;
    let status = crate::services::timeline_service::TimelineService::get_inactivity_status(&timeline);
    Ok(serde_json::json!({
        "hoursSinceLastInteraction": status.hours_since_last_interaction,
        "minutesSinceLastInteraction": status.minutes_since_last_interaction,
        "isInactive": status.is_inactive,
        "isVeryInactive": status.is_very_inactive,
    }))
}

#[tauri::command]
pub fn get_emotion_history(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let ghost_id = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().map(|g| g.ghost_id.clone())
    };

    let mut events = Vec::new();

    if let Some(ref gid) = ghost_id {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let db_events = db.load_emotional_events(gid, 50)?;
            for e in &db_events {
                events.push(serde_json::json!({
                    "eventType": e.event_type,
                    "intensity": e.intensity,
                    "loveHateDelta": e.love_hate_delta,
                    "baselineDelta": e.baseline_delta,
                    "timestamp": e.created_at,
                    "description": e.description,
                }));
            }
        }
    }

    if events.is_empty() {
        let timeline = state.timeline.lock().map_err(|e| e.to_string())?;
        for e in &timeline.event_manager.events {
            events.push(serde_json::json!({
                "eventType": format!("{:?}", e.event_type),
                "intensity": e.intensity,
                "loveHateDelta": e.love_hate_delta,
                "baselineDelta": e.baseline_delta,
                "timestamp": e.timestamp.to_rfc3339(),
                "description": e.description,
            }));
        }
    }

    Ok(serde_json::json!({
        "events": events,
        "total": events.len(),
    }))
}

#[tauri::command]
pub fn record_interaction(state: State<'_, AppState>) -> Result<(), String> {
    let mut timeline = state.timeline.lock().map_err(|e| e.to_string())?;
    timeline.record_interaction();
    
    // 保存 last_interaction 到数据库
    let now = chrono::Utc::now().to_rfc3339();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(db) = db_guard.as_ref() {
        let _ = db.save_last_interaction(now);
    }
    
    Ok(())
}

fn build_system_prompt(ghost: &Ghost, state: &State<'_, AppState>) -> Result<String, String> {
    let budget = TokenBudget::default();

    let memories = if let Some(db) = state.db.lock().map_err(|e| e.to_string())?.as_ref() {
        let mem_service = MemoryService::new(db, &ghost.ghost_id);
        mem_service.get_memories_for_context(budget.fillable_memory_budget())?
    } else {
        vec![]
    };

    let experiences = if let Some(db) = state.db.lock().map_err(|e| e.to_string())?.as_ref() {
        let mem_service = MemoryService::new(db, &ghost.ghost_id);
        mem_service.get_experiences()?
    } else {
        vec![]
    };

    Ok(PromptBuilder::build_system_prompt(
        &ghost.soul,
        &memories,
        &experiences,
        &ghost.name,
    ))
}

fn truncate(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

#[tauri::command]
pub async fn generate_image(
    prompt: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref()
            .ok_or("AI 接口未配置。请先在「配置 AI 接口」中填写 Endpoint、API Key 和 Model。")?
            .clone()
    };

    let ghost_name = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().map(|g| g.name.clone()).unwrap_or_else(|| "桌宠".into())
    };

    let enriched_prompt = format!(
        "{}想画一幅画：{}",
        ghost_name, prompt
    );

    let ai_service = AIService::new(ai_config);
    let result = ai_service.generate_image(&enriched_prompt).await?;

    if result.starts_with("url:") {
        Ok(serde_json::json!({
            "type": "url",
            "data": &result[4..],
        }))
    } else {
        Ok(serde_json::json!({
            "type": "base64",
            "data": result,
        }))
    }
}

#[tauri::command]
pub fn ensure_database(state: State<'_, AppState>) -> Result<(), String> {
    let mut locked = state.db.lock().map_err(|e| e.to_string())?;
    if locked.is_some() {
        // 已经初始化，尝试加载 lastInteraction
        if let Some(ref db_ref) = *locked {
            if let Some(ts) = db_ref.load_last_interaction() {
                if let Ok(mut timeline) = state.timeline.lock() {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&ts) {
                        timeline.last_interaction = dt.with_timezone(&chrono::Utc);
                    }
                }
            }
        }
        return Ok(());
    }
    let app_data_dir = dirs::data_dir().ok_or("无法获取应用数据目录")?;
    let db_path = app_data_dir.join("DeskPet").join("deskpet.db");
    if let Some(p) = db_path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    let db = Database::new(db_path.to_str().ok_or("数据库路径无效")?)?;
    *locked = Some(db);

    // 加载 lastInteraction 并设置到 timeline
    if let Some(ref db_ref) = *locked {
        if let Some(ts) = db_ref.load_last_interaction() {
            if let Ok(mut timeline) = state.timeline.lock() {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&ts) {
                    timeline.last_interaction = dt.with_timezone(&chrono::Utc);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn save_chat_message(
    ghost_id: String,
    role: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    db.save_chat_message(&id, &ghost_id, &role, &content)?;
    Ok(id)
}

#[tauri::command]
pub fn load_chat_history(
    ghost_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    let limit = limit.unwrap_or(100);
    let messages = db.load_chat_history(&ghost_id, limit)?;
    Ok(messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "createdAt": m.created_at,
            })
        })
        .collect())
}

#[tauri::command]
pub fn clear_chat_history(
    ghost_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    db.clear_chat_history(&ghost_id)
}

/// P2-7: Edge TTS 语音合成 — 免费 Microsoft 端点，自然中文语音
#[tauri::command]
pub async fn speak_edge_tts(
    text: String,
    voice: Option<String>,
) -> Result<String, String> {
    let tts = crate::services::tts_service::EdgeTTS::new();
    let voice_name = voice.unwrap_or_else(|| "zh-CN-XiaoxiaoNeural".to_string());
    let audio_bytes = tts.synthesize(&text, &voice_name).await?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&audio_bytes))
}