use crate::commands::chat_commands::AppState;
use crate::services::ai_service::AIService;
use crate::services::screenshot_service::ScreenshotService;
use crate::core::soul::impression::ImpressionEvent;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[tauri::command]
pub async fn capture_screenshot() -> Result<String, String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::task::spawn_blocking(|| ScreenshotService::capture_screen()),
    )
    .await
    .map_err(|_| "截图超时（15秒），请重试".into())
    .and_then(|r| r.map_err(|e| format!("截图任务失败: {}", e)))
    .and_then(|r| r)
}

#[tauri::command]
pub fn store_screenshot_data(data: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut screenshot_data = state.screenshot_data.lock().map_err(|e| e.to_string())?;
    *screenshot_data = Some(data);
    Ok(())
}

#[tauri::command]
pub fn get_screenshot_data(state: State<'_, AppState>) -> Result<String, String> {
    let mut screenshot_data = state.screenshot_data.lock().map_err(|e| e.to_string())?;
    screenshot_data.take().ok_or("No screenshot data available".into())
}

#[tauri::command]
pub fn close_screenshot_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("screenshot-overlay") {
        let _ = window.close();
    }
}

#[tauri::command]
pub async fn analyze_screenshot(
    image_base64: String,
    question: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref()
            .ok_or("AI 接口未配置。请先在「配置 AI 接口」中填写 Endpoint、API Key 和 Model。")?
            .clone()
    };

    let ghost = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref()
            .ok_or("没有加载 Ghost，请先生成桌宠灵魂")?
            .clone()
    };

    let name = ghost.name.clone();
    let love_hate = ghost.soul.sensibility.love_hate;
    let identity = ghost.persona.clone().unwrap_or_else(|| "一个有感情、有个性的桌宠".to_string());

    let personality_desc = {
        let dims = &ghost.soul.innate_tendency.self_dims;
        format!(
            "开放{:.0}%, 尽责{:.0}%, 外向{:.0}%, 宜人{:.0}%, 情绪{:.0}%, 创造{:.0}%",
            dims.openness * 100.0,
            dims.conscientiousness * 100.0,
            dims.extraversion * 100.0,
            dims.agreeableness * 100.0,
            dims.neuroticism * 100.0,
            dims.creativity * 100.0
        )
    };

    let system_prompt = format!(
        "你是{}，{}。你的主人刚截取了屏幕上的一张图片，想让你看看。\
         请用你自己的视角和语气描述你看到的内容，保持好奇和有趣。\
         你的性格倾向：{}。当前对主人的好感度：{:.1}/100。\
         用简短的中文回复，像一个活生生的伙伴在观察主人的屏幕。",
        name, identity, personality_desc, love_hate
    );

    let default_question = "请看看这张截图，告诉我你看到了什么？你对这些内容有什么想法？";
    let question_text = question.as_deref().unwrap_or(default_question);

    let ai_service = AIService::new(ai_config);
    let response = ai_service.chat_with_image_base64(&system_prompt, &image_base64, question_text, &[]).await?;

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            // 保存用户的截图消息（含文字），确保重启后对话历史完整
            let user_msg_id = Uuid::new_v4().to_string();
            let user_content = if question.is_some() {
                format!("📷 {}", question_text)
            } else {
                "📷 截图".to_string()
            };
            if let Err(e) = db.save_chat_message(&user_msg_id, &ghost.ghost_id, "user", &user_content) {
                eprintln!("[screenshot] 保存用户截图消息失败: {}", e);
            }
            // 保存桌宠的回复
            let pet_msg_id = Uuid::new_v4().to_string();
            if let Err(e) = db.save_chat_message(&pet_msg_id, &ghost.ghost_id, "assistant", &response) {
                eprintln!("[screenshot] 保存截图分析消息失败: {}", e);
            }
        }
    }

    Ok(serde_json::json!({
        "description": response,
        "petName": name,
    }))
}

#[tauri::command]
pub async fn curiosity_background_analyze(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref().cloned()
    };

    let ai_config = match ai_config {
        Some(c) => c,
        None => return Ok(serde_json::json!({ "analyzed": false, "reason": "AI not configured" })),
    };

    let ghost = {
        let locked = state.ghost.lock().map_err(|e| e.to_string())?;
        locked.as_ref().cloned()
    };

    let ghost = match ghost {
        Some(g) => g,
        None => return Ok(serde_json::json!({ "analyzed": false, "reason": "No ghost" })),
    };

    if !matches!(ghost.soul.curiosity.level, crate::core::soul::curiosity::CuriosityLevel::Enhanced) {
        return Ok(serde_json::json!({ "analyzed": false, "reason": "Not in Enhanced mode" }));
    }

    let base64 = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::task::spawn_blocking(|| ScreenshotService::capture_screen()),
    )
    .await
    .map_err(|_| "截图超时".into())
    .and_then(|r| r.map_err(|e| format!("截图失败: {}", e)))
    .and_then(|r| r)?;

    let system_prompt = format!(
        "你是{}，一个桌宠。你正在后台好奇地观察主人的屏幕。\
         请用一句话简要描述主人似乎在做什么（如'主人正在看技术文档'或'主人在玩游戏'）。\
         同时评估主人此刻表现出的性格倾向，按6个维度各给-1到+1的值。\
         严格按JSON格式回复：{{\"activity\": \"...\", \"openness\": 0, \"conscientiousness\": 0, \"extraversion\": 0, \"agreeableness\": 0, \"neuroticism\": 0, \"creativity\": 0}}",
        ghost.name
    );

    let ai_service = AIService::new(ai_config);
    let raw = ai_service.chat_with_image_base64(&system_prompt, &base64, "请观察这张屏幕截图", &[]).await?;

    let json_str = {
        let start = raw.find('{');
        let end = raw.rfind('}');
        match (start, end) {
            (Some(s), Some(e)) if e > s => raw[s..=e].to_string(),
            _ => return Ok(serde_json::json!({ "analyzed": false, "reason": "Parse error" })),
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[curiosity] JSON解析失败: {}, 原始: {}", e, &json_str[..json_str.len().min(200)]);
            return Ok(serde_json::json!({ "analyzed": false, "reason": "JSON parse error" }));
        }
    };
    let activity = parsed["activity"].as_str().unwrap_or("主人正在使用电脑").to_string();

    {
        let mut locked = state.ghost.lock().map_err(|e| e.to_string())?;
        if let Some(g) = locked.as_mut() {
            let imp_event = ImpressionEvent {
                openness_delta: parsed["openness"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                conscientiousness_delta: parsed["conscientiousness"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                extraversion_delta: parsed["extraversion"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                agreeableness_delta: parsed["agreeableness"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                neuroticism_delta: parsed["neuroticism"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                creativity_delta: parsed["creativity"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0),
                snippet: Some(format!("(好奇观察) {}", activity)),
            };
            g.soul.impression.apply_event(&imp_event, &g.soul.innate_tendency.preferred_dims);
        }
    }

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(db) = db_guard.as_ref() {
            let ghost_snapshot = {
                let locked = state.ghost.lock().map_err(|e| e.to_string())?;
                locked.as_ref().cloned()
            };
            if let Some(g) = ghost_snapshot {
                let imp = &g.soul.impression;
                let snippets_json = serde_json::to_string(&imp.general_impression_snippets).unwrap_or_else(|_| "[]".into());
                if let Err(e) = db.save_impression(
                    &g.ghost_id,
                    imp.openness_score,
                    imp.conscientiousness_score,
                    imp.extraversion_score,
                    imp.agreeableness_score,
                    imp.neuroticism_score,
                    imp.creativity_score,
                    imp.overall_affinity,
                    &snippets_json,
                ) {
                    eprintln!("[curiosity] 保存印象数据失败: {}", e);
                }
            }
        }
    }

    Ok(serde_json::json!({
        "analyzed": true,
        "activity": activity,
    }))
}