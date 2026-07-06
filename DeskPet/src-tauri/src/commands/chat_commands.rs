use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, State};

use crate::core::ghost::Ghost;
use crate::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType};
use crate::core::prompt::prompt_builder::{AnsweringMode, PromptBuilder};
use crate::core::prompt::token_budget::TokenBudget;
use crate::data::database::{Database, DocumentKnowledgeRow};
use crate::services::ai_service::{AIService, AIProviderConfig, ChatMessage};
use crate::services::memory_service::MemoryService;
use crate::core::memory::local_compressor;
use crate::services::sentiment_service::{parse_event_type, SentimentAnalyzer};
use crate::services::memory_compressor::MemoryCompressor;
use crate::services::timeline_service::TimelineState;
use crate::services::weather_service::WeatherCache;

pub struct AppState {
    pub ghost: Mutex<Option<Ghost>>,
    pub db: Mutex<Option<Database>>,
    pub ai_config: Mutex<Option<AIProviderConfig>>,
    pub timeline: Mutex<TimelineState>,
    pub screenshot_data: Mutex<Option<String>>,
    pub answering_mode: Mutex<AnsweringMode>,
    pub weather: Mutex<WeatherCache>,
}

#[tauri::command]
pub async fn chat_with_pet(
    message: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let ghost = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().ok_or("没有加载 Ghost，请先生成桌宠灵魂")?.clone()
    };

    // 刷新天气缓存（异步，不阻塞对话）
    let weather_context = crate::services::weather_service::refresh_weather_if_stale(&state.weather).await;

    let system_prompt = build_system_prompt(&ghost, &state, &weather_context)?;

    let mut messages = Vec::new();

    let is_first_conversation: bool;
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let history = db.load_chat_history(&ghost.ghost_id, 20)?;
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
    let app_handle_clone = app_handle.clone();

    let raw_response = match ai_service.chat_streaming(
        &system_prompt,
        &messages,
        |token| {
            let _ = app_handle_clone.emit("chat:token", token);
        },
    ).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[流式] 首次尝试失败: {}，将重试（非流式）", e);
            let ai_service = AIService::new({
                let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
                locked.as_ref().ok_or("AI 接口未配置")?.clone()
            });
            ai_service.chat(&system_prompt, &messages).await?
        }
    };

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

    // 保存用户消息和 AI 回复到 ChatMessages，确保对话上下文完整持久化
    // 用毫秒精度时间戳避免同秒内 user/pet 排序不稳定
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let user_ts = format_ms_iso8601(now_ms);
            let pet_ts = format_ms_iso8601(now_ms + 1);

            let user_msg_id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = db.save_chat_message_with_ts(&user_msg_id, &ghost.ghost_id, "user", &message, Some(&user_ts)) {
                eprintln!("[chat] 保存用户消息失败: {}", e);
            }
            let pet_msg_id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = db.save_chat_message_with_ts(&pet_msg_id, &ghost.ghost_id, "assistant", &response, Some(&pet_ts)) {
                eprintln!("[chat] 保存AI回复失败: {}", e);
            }
        }
    }

    // 发送 chat:complete — 前端显示完整回复
    let mut complete_payload = serde_json::json!({
        "response": response,
    });
    if let Some(ref img) = generated_image {
        complete_payload["generatedImage"] = serde_json::Value::String(img.clone());
    }
    let _ = app_handle.emit("chat:complete", complete_payload);

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

            if let Err(e) = mem_service.add_short_term_memory(
                &format!("用户说：{}", compressed_user.summary),
                0.4,
                0.5,
                user_sentiment,
                Some("chat"),
                user_entities.as_deref(),
            ) {
                eprintln!("[chat] 保存用户短期记忆失败: {}", e);
            }

            let compressed_response = local_compressor::local_compress_message(&response);
            let response_sentiment = compressed_response.sentiment;
            let response_entities = serde_json::to_string(&compressed_response.entities).ok();

            let response_summary = if compressed_response.summary.len() > 200 {
                let ts: String = compressed_response.summary.chars().take(200).collect();
                format!("{}…", ts)
            } else {
                compressed_response.summary.clone()
            };
            if let Err(e) = mem_service.add_short_term_memory(
                &format!("你回复：{}", response_summary),
                0.4,
                0.5,
                response_sentiment,
                Some("chat"),
                response_entities.as_deref(),
            ) {
                eprintln!("[chat] 保存AI回复短期记忆失败: {}", e);
            }

            if let Ok(experiences) = db.get_experiences(&ghost_snapshot.ghost_id) {
                let response_lower = response.to_lowercase();
                for exp in &experiences {
                    let name_lower = exp.name.to_lowercase();
                    if response_lower.contains(&name_lower) {
                        let new_proficiency = (exp.proficiency + 0.01).min(1.0);
                        if let Err(e) = db.update_experience_proficiency(&exp.id, new_proficiency) {
                            eprintln!("[chat] 更新经验熟练度失败: {}", e);
                        }
                    }
                }
            }
        }
    }

    // 发送 chat:post-processed — 前端更新 ghost 状态、释放 chatLoading
    let _ = app_handle.emit("chat:post-processed", serde_json::json!({
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
    }));

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
            if let Err(e) = db.save_emotional_event(
                &event_id,
                &ghost_snapshot.ghost_id,
                &event_type_str,
                sentiment_love_hate.abs() / 5.0,
                sentiment_love_hate,
                emotional_event.baseline_delta,
                Some(&truncate(&message, 100)),
            ) {
                eprintln!("[chat] 保存情感事件失败: {}", e);
            }

            let imp = &ghost_snapshot.soul.impression;
            let snippets_json = serde_json::to_string(&imp.general_impression_snippets).unwrap_or_else(|_| "[]".into());
            if let Err(e) = db.save_impression(
                &ghost_snapshot.ghost_id,
                imp.openness_score,
                imp.conscientiousness_score,
                imp.extraversion_score,
                imp.agreeableness_score,
                imp.neuroticism_score,
                imp.creativity_score,
                imp.overall_affinity,
                &snippets_json,
            ) {
                eprintln!("[chat] 保存印象数据失败: {}", e);
            }

            let chunk_id = uuid::Uuid::new_v4().to_string();
            let chunk_summary = format!("{}: {} → {}", ghost_snapshot.name, truncate(&message, 40), truncate(&response, 40));
            if let Err(e) = db.save_conversation_chunk(
                &chunk_id,
                &ghost_snapshot.ghost_id,
                &chunk_summary,
                if sentiment_love_hate.abs() > 3.0 { 0.7 } else { 0.4 },
                None,
            ) {
                eprintln!("[chat] 保存对话片段失败: {}", e);
            }
        }
    }

    if let Ok(mut timeline) = state.timeline.lock() {
        timeline.record_interaction();
    }

    Ok(())
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

    {
        let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if db_guard.is_none() {
            let app_data_dir = dirs::data_dir()
                .ok_or("无法获取应用数据目录")?;
            let db_path = app_data_dir.join("DeskPet").join("deskpet.db");
            std::fs::create_dir_all(db_path.parent().ok_or("无法创建数据库目录")?)
                .map_err(|e| format!("创建目录失败: {}", e))?;
            let db = Database::new(db_path.to_str().ok_or("数据库路径无效")?)?;
            *db_guard = Some(db);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn configure_weather(
    api_key: String,
    city: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::services::weather_service::WeatherConfig;
    let config = WeatherConfig { api_key, city };
    let mut locked = state.weather.lock().map_err(|e| e.to_string())?;
    locked.config = config;
    // 强制下次刷新
    locked.last_fetch = None;
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
pub fn set_answering_mode(
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let new_mode = match mode.as_str() {
        "Companion" => AnsweringMode::Companion,
        "Assistant" => AnsweringMode::Assistant,
        _ => return Err(format!(
            "Unknown answering mode: {}. Use Companion or Assistant",
            mode
        )),
    };

    let mut locked = state.answering_mode.lock().map_err(|e| e.to_string())?;
    *locked = new_mode.clone();

    Ok(new_mode.to_string())
}

#[tauri::command]
pub fn get_answering_mode(state: State<'_, AppState>) -> Result<String, String> {
    let locked = state.answering_mode.lock().map_err(|e| e.to_string())?;
    Ok(locked.to_string())
}

#[tauri::command]
pub fn timeline_tick(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut rng = rand::thread_rng();

    let tick_result;
    let ghost_id: String;
    let inactivity_event_opt: Option<crate::services::timeline_service::InactivityEventResult>;

    {
        let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let ghost = locked.as_mut().ok_or("No ghost loaded")?;
        ghost_id = ghost.ghost_id.clone();

        let mut timeline = state.timeline.lock().map_err(|e| e.to_string())?;
        tick_result = crate::services::timeline_service::TimelineService::tick(
            ghost, &mut timeline, &mut rng,
        );
        inactivity_event_opt = tick_result.inactivity_event.clone();
    }

    let mut decayed_experiences = 0usize;
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let mem_service = MemoryService::new(db, &ghost_id);
            decayed_experiences = mem_service.decay_experience_proficiency().unwrap_or(0);

            if let Some(ref inactivity_evt) = inactivity_event_opt {
                let description = format!("主人已经{}小时没有互动了...", inactivity_evt.hours_away);
                if let Err(e) = db.save_emotional_event(
                    &uuid::Uuid::new_v4().to_string(),
                    &ghost_id,
                    &inactivity_evt.event_type,
                    inactivity_evt.intensity,
                    inactivity_evt.love_hate_delta,
                    0.0,
                    Some(&description),
                ) {
                    eprintln!("[timeline_tick] 保存不活跃事件失败: {}", e);
                }
            }
        }
    }
    if decayed_experiences > 0 {
        eprintln!("[经验衰减] {}条经验熟练度已衰减", decayed_experiences);
    }

            Ok(serde_json::json!({
                "loveHate": tick_result.love_hate_after_tick,
                "baseline": tick_result.baseline_after_tick,
                "inactivityEvent": tick_result.inactivity_event,
                "curiosityTriggered": tick_result.curiosity_triggered,
                "sleepTriggered": tick_result.sleep_triggered,
                "diaryTriggered": tick_result.diary_triggered,
            }))
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
    {
        let mut timeline = state.timeline.lock().map_err(|e| e.to_string())?;
        timeline.record_interaction();
    }
    
    let now = chrono::Utc::now().to_rfc3339();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(db) = db_guard.as_ref() {
        if let Err(e) = db.save_last_interaction(now) {
            eprintln!("[record_interaction] 保存last_interaction失败: {}", e);
        }
    }
    
    Ok(())
}

fn build_system_prompt(ghost: &Ghost, state: &State<'_, AppState>, weather_context: &Option<String>) -> Result<String, String> {
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

    let answering_mode = state.answering_mode.lock().map_err(|e| e.to_string())?.clone();

    let mut prompt = PromptBuilder::build_system_prompt(
        &ghost.soul,
        &memories,
        &experiences,
        &ghost.name,
        &answering_mode,
        &ghost.persona,
    );

    if let Some(w) = weather_context {
        prompt.push_str("\n\n");
        prompt.push_str(w);
    }

    if let Some(db) = state.db.lock().map_err(|e| e.to_string())?.as_ref() {
        if let Ok(docs) = db.get_ready_documents() {
            if !docs.is_empty() {
                let doc_ctx = build_document_context(&docs);
                if !doc_ctx.is_empty() {
                    prompt.push_str("\n\n");
                    prompt.push_str(&doc_ctx);
                }
            }
        }
    }

    Ok(prompt)
}

fn truncate(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

fn build_document_context(docs: &[DocumentKnowledgeRow]) -> String {
    let mut parts: Vec<String> = vec![];

    for doc in docs {
        if let Some(ref summary_json) = doc.summary_json {
            if let Ok(summary) = serde_json::from_str::<serde_json::Value>(summary_json) {
                let mut lines: Vec<String> = vec![];

                if let Some(one_liner) = summary["one_liner"].as_str() {
                    if !one_liner.is_empty() {
                        lines.push(format!("- 概要：{}", one_liner));
                    }
                }

                if let Some(characters) = summary["characters"].as_array() {
                    if !characters.is_empty() {
                        let chars_desc: Vec<String> = characters.iter().filter_map(|c| {
                            let name = c["name"].as_str().unwrap_or("?");
                            let role = c["role"].as_str().unwrap_or("");
                            let label = if role.is_empty() { name.to_string() } else { format!("{}（{}）", name, role) };
                            Some(label)
                        }).collect();
                        lines.push(format!("- 主要角色：{}", chars_desc.join("、")));
                    }
                }

                if let Some(themes) = summary["themes"].as_array() {
                    if !themes.is_empty() {
                        let theme_strs: Vec<&str> = themes.iter().filter_map(|t| t.as_str()).collect();
                        lines.push(format!("- 核心主题：{}", theme_strs.join("、")));
                    }
                }

                if !lines.is_empty() {
                    parts.push(format!("主人有一部作品《{}》：\n{}", doc.title, lines.join("\n")));
                }
            }
        }
    }

    if parts.is_empty() {
        return String::new();
    }

    format!(
        "## 你了解的作品\n{}\n\n（注意：如果主人没有主动提起作品相关内容，不要突兀地评价这些作品。当主人聊到时，可以自然地结合你对作品的了解来回应。）",
        parts.join("\n\n")
    )
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
    let last_interaction_ts: Option<String>;
    {
        let mut locked = state.db.lock().map_err(|e| e.to_string())?;
        if locked.is_some() {
            last_interaction_ts = locked.as_ref().and_then(|db_ref| db_ref.load_last_interaction());
            if let Some(ref ts) = last_interaction_ts {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                    if let Ok(mut timeline) = state.timeline.lock() {
                        timeline.last_interaction = dt.with_timezone(&chrono::Utc);
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
        last_interaction_ts = db.load_last_interaction();
        *locked = Some(db);
    }

    if let Some(ts) = last_interaction_ts {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&ts) {
            if let Ok(mut timeline) = state.timeline.lock() {
                timeline.last_interaction = dt.with_timezone(&chrono::Utc);
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
pub fn repair_chat_history_order(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_mut().ok_or("数据库未初始化")?;
    db.repair_chat_history_order()
}

#[tauri::command]
pub fn purge_welcome_messages(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_mut().ok_or("数据库未初始化")?;
    let mut total = 0;
    total += db.delete_system_messages_like("%灵魂已恢复%")?;
    total += db.delete_system_messages_like("%灵魂已注入%")?;
    Ok(total)
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

/// P24: 成就系统 — 检查并解锁新成就
#[tauri::command]
pub fn check_achievements(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (ghost_id, generation, love_hate) = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let g = locked.as_ref().ok_or("No ghost loaded")?;
        (g.ghost_id.clone(), g.generation, g.soul.sensibility.love_hate)
    };

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;

    let mut new_achievements: Vec<serde_json::Value> = Vec::new();

    let chat_count = db.count_chat_messages(&ghost_id).unwrap_or(0);
    let dream_count = db.count_dreams(&ghost_id).unwrap_or(0);

    let checks: Vec<(&str, bool, &str)> = vec![
        ("first_chat", chat_count >= 2, "💬 初次对话 — 完成了第一次交流"),
        ("chat_10", chat_count >= 20, "💬 话痨 — 累计20条对话"),
        ("chat_100", chat_count >= 200, "💬 挚友 — 累计200条对话"),
        ("love_50", love_hate >= 50.0, "❤️ 暖心 — 好感度突破50"),
        ("love_80", love_hate >= 80.0, "💕 亲密 — 好感度突破80"),
        ("transfer_1", generation > 1, "🔄 新生 — 完成第一次灵魂传送"),
        ("dream_first", dream_count >= 1, "💤 初梦 — 第一次做梦"),
    ];

    for (key, condition, _desc) in &checks {
        if *condition {
            let id = uuid::Uuid::new_v4().to_string();
            if db.save_achievement(&id, &ghost_id, key)? {
                new_achievements.push(serde_json::json!({
                    "key": key,
                    "name": get_achievement_name(key),
                    "description": get_achievement_desc(key),
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "newAchievements": new_achievements,
    }))
}

fn get_achievement_name(key: &str) -> &str {
    match key {
        "first_chat" => "初次对话",
        "chat_10" => "话痨",
        "chat_100" => "挚友",
        "love_50" => "暖心",
        "love_80" => "亲密",
        "transfer_1" => "新生",
        "dream_first" => "初梦",
        _ => "未知成就",
    }
}

fn get_achievement_desc(key: &str) -> &str {
    match key {
        "first_chat" => "完成了第一次交流",
        "chat_10" => "累计20条对话",
        "chat_100" => "累计200条对话",
        "love_50" => "好感度突破50",
        "love_80" => "好感度突破80",
        "transfer_1" => "完成第一次灵魂传送",
        "dream_first" => "第一次做梦",
        _ => "",
    }
}

/// P24: 每日日记 — 基于当天记忆生成宠物视角日记
#[tauri::command]
pub async fn generate_diary(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (ghost_id, ghost_name, love_hate) = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        let g = locked.as_ref().ok_or("No ghost loaded")?;
        (g.ghost_id.clone(), g.name.clone(), g.soul.sensibility.love_hate)
    };

    let ai_config = {
        state.ai_config.lock().map_err(|e| e.to_string())?
            .clone()
            .ok_or("AI not configured")?
    };

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let memories: Vec<String> = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let stm = db.get_short_term_memories(&ghost_id, 15).unwrap_or_default();
            let events = db.load_emotional_events(&ghost_id, 10).unwrap_or_default();
            let mut snippets: Vec<String> = stm.iter().map(|m| m.summary.clone()).collect();
            for e in &events {
                if let Some(ref desc) = e.description {
                    snippets.push(format!("[情感事件: {}] {}", e.event_type, desc));
                }
            }
            snippets
        } else {
            Vec::new()
        }
    };

    let memory_text = if memories.is_empty() {
        "（今天还没有任何记忆）".to_string()
    } else {
        memories.iter().enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mood = if love_hate > 30.0 { "愉快" } else if love_hate > 0.0 { "平静" } else { "低落" };

    let system_prompt = format!(
        "你是{}，一个桌宠精灵。请根据以下今天的记忆碎片，以宠物口吻写一篇今天的简短日记（~100字中文）。\n\
         今天你的整体心情：{}。\n\
         风格：第一人称、私密、像对自己说话。可以提到主人、提到情绪变化。\n\n\
         今天的记忆：\n{}",
        ghost_name, mood, memory_text
    );

    let ai_service = AIService::new(ai_config);
    let diary_text = ai_service.chat(
        "你是日记生成系统。只输出日记内容，不要任何前缀说明。",
        &vec![ChatMessage { role: "user".into(), content: system_prompt }],
    ).await?;

    let diary_id = uuid::Uuid::new_v4().to_string();

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            if let Err(e) = db.save_diary(&diary_id, &ghost_id, diary_text.trim(), &today) {
                eprintln!("[日记] 保存失败: {}", e);
            }
        }
    }

    {
        if let Ok(mut timeline) = state.timeline.lock() {
            timeline.last_diary_time = chrono::Utc::now();
        }
    }

    Ok(serde_json::json!({
        "diaryId": diary_id,
        "entryDate": today,
        "diaryText": diary_text.trim(),
    }))
}

#[tauri::command]
pub fn get_diary_entries(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let ghost_id = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().map(|g| g.ghost_id.clone()).ok_or("No ghost loaded")?
    };

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;

    let rows = db.get_diary_entries(&ghost_id, 30)?;
    let entries: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "entryDate": r.entry_date,
            "summary": r.summary,
        })
    }).collect();

    Ok(serde_json::json!({
        "entries": entries,
        "total": entries.len(),
    }))
}

#[tauri::command]
pub fn get_achievements(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let ghost_id = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().map(|g| g.ghost_id.clone()).ok_or("No ghost loaded")?
    };

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;

    let rows = db.get_achievements(&ghost_id)?;
    let achievements: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "key": r.achievement_key,
            "name": get_achievement_name(&r.achievement_key),
            "description": get_achievement_desc(&r.achievement_key),
            "unlockedAt": r.unlocked_at,
        })
    }).collect();

    Ok(serde_json::json!({
        "achievements": achievements,
        "total": achievements.len(),
    }))
}

/// 将毫秒级 Unix 时间戳格式化为带毫秒的 ISO8601 字符串（UTC），如 "2026-07-06 12:34:56.789"
/// 用于 ChatMessages.CreatedAt，确保同秒内的消息可按毫秒精确排序。
fn format_ms_iso8601(ms: i64) -> String {
    let secs = ms / 1000;
    let millis = ms % 1000;
    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let h = rem_secs / 3600;
    let mi = (rem_secs % 3600) / 60;
    let s = rem_secs % 60;
    let civil = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        civil.0, civil.1, civil.2, h, mi, s, millis
    )
}

/// 将"自 1970-01-01 起的天数"转换为 (year, month, day)。
fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let d = days + 719468;
    let era = if d >= 0 { d / 146097 } else { (d - 146096) / 146097 };
    let doe = d - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yy = if m <= 2 { y + 1 } else { y };
    let dd = doy - (153 * mp + 2) / 5 + 1;
    (yy, m, dd)
}