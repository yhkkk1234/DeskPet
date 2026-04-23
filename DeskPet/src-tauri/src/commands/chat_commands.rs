use std::sync::Mutex;
use tauri::State;

use crate::core::ghost::Ghost;
use crate::core::soul::emotional_event::EmotionalEvent;
use crate::core::prompt::prompt_builder::PromptBuilder;
use crate::core::prompt::token_budget::TokenBudget;
use crate::data::database::Database;
use crate::services::ai_service::{AIService, AIProviderConfig, ChatMessage};
use crate::services::memory_service::MemoryService;
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

    let messages = vec![ChatMessage {
        role: "user".into(),
        content: message.clone(),
    }];

    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref()
            .ok_or("AI 接口未配置。请点击右侧面板「配置 AI 接口」按钮，填写 Endpoint、API Key 和 Model 后重试。")?
            .clone()
    };

    let ai_service = AIService::new(ai_config);
    let response = ai_service.chat(&system_prompt, &messages).await?;

    let event_type_str: String;
    let sentiment_love_hate: f64;
    let sentiment_snippet: Option<String>;
    let impression_deltas: (f64, f64, f64, f64, f64, f64);

    match SentimentAnalyzer::analyze(&ai_service, &message, &response, &ghost.name).await {
        Ok(result) => {
            event_type_str = result.event_type.clone();
            sentiment_love_hate = result.love_hate_hint;
            sentiment_snippet = result.impression.snippet.clone();
            impression_deltas = (
                result.impression.openness_delta,
                result.impression.conscientiousness_delta,
                result.impression.extraversion_delta,
                result.impression.agreeableness_delta,
                result.impression.neuroticism_delta,
                result.impression.creativity_delta,
            );

            let emotional_event = EmotionalEvent::new_with_description(
                parse_event_type(&result.event_type),
                result.intensity,
                format!("对话: {} -> {}", truncate(&message, 50), truncate(&response, 50)),
            );

            if let Ok(mut locked) = state.ghost.lock() {
                if let Some(g) = locked.as_mut() {
                    let mut rng = rand::thread_rng();
                    g.soul.apply_emotional_event(&emotional_event, &mut rng);
                }
            }
        }
        Err(e) => {
            eprintln!("[情感分析] 分析失败（不影响对话）: {}", e);
            event_type_str = "NormalChat".into();
            sentiment_love_hate = 0.0;
            sentiment_snippet = None;
            impression_deltas = (0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        }
    }

    if let Ok(mut locked) = state.ghost.lock() {
        if let Some(g) = locked.as_mut() {
            let mut rng = rand::thread_rng();
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

            if sentiment_love_hate != 0.0 {
                g.soul.sensibility.apply_event(sentiment_love_hate);
            }
            g.soul.sensibility.apply_spring(&mut rng);
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
            let _ = mem_service.add_short_term_memory(
                &format!("用户说：{}", message),
                0.4,
                0.5,
                0.0,
                Some("chat"),
                None,
            );
            let response_summary = truncate(&response, 200);
            let _ = mem_service.add_short_term_memory(
                &format!("你回复：{}", response_summary),
                0.4,
                0.5,
                sentiment_love_hate,
                Some("chat"),
                None,
            );
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

    if let Ok(mut timeline) = state.timeline.lock() {
        timeline.record_interaction();
    }

    Ok(serde_json::json!({
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
    }))
}

#[tauri::command]
pub fn configure_ai(
    endpoint: String,
    api_key: String,
    model: String,
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
        vision_model: None,
        image_model: None,
        is_default: true,
    };

    {
        let mut locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        *locked = Some(config);
    }

    if state.db.lock().map_err(|e| e.to_string())?.is_none() {
        let app_data_dir = dirs::data_dir()
            .ok_or("无法获取应用数据目录")?;
        let db_path = app_data_dir.join("deskpet").join("deskpet.db");
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
    let timeline = state.timeline.lock().map_err(|e| e.to_string())?;
    let events: Vec<serde_json::Value> = timeline.event_manager.events.iter()
        .map(|e| serde_json::json!({
            "eventType": format!("{:?}", e.event_type),
            "intensity": e.intensity,
            "loveHateDelta": e.love_hate_delta,
            "baselineDelta": e.baseline_delta,
            "timestamp": e.timestamp.to_rfc3339(),
            "description": e.description,
        }))
        .collect();
    Ok(serde_json::json!({
        "events": events,
        "total": events.len(),
    }))
}

#[tauri::command]
pub fn record_interaction(state: State<'_, AppState>) -> Result<(), String> {
    let mut timeline = state.timeline.lock().map_err(|e| e.to_string())?;
    timeline.record_interaction();
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