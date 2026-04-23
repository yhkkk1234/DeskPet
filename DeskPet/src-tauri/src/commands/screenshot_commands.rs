use crate::commands::chat_commands::AppState;
use crate::services::ai_service::AIService;
use crate::services::screenshot_service::ScreenshotService;
use tauri::{AppHandle, Manager, State};

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
pub async fn capture_region(x: i32, y: i32, width: i32, height: i32) -> Result<String, String> {
    tokio::task::spawn_blocking(move || ScreenshotService::capture_region(x, y, width, height))
    .await
    .map_err(|e| format!("截图任务失败: {}", e))
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
        "你是{}，一个有感情、有个性的桌宠。你的主人刚截取了屏幕上的一张图片，想让你看看。\
         请用你自己的视角和语气描述你看到的内容，保持好奇和有趣。\
         你的性格倾向：{}。当前对主人的好感度：{:.1}/100。\
         用简短的中文回复，像一个活生生的伙伴在观察主人的屏幕。",
        name, personality_desc, love_hate
    );

    let default_question = "请看看这张截图，告诉我你看到了什么？你对这些内容有什么想法？";
    let question_text = question.as_deref().unwrap_or(default_question);

    let ai_service = AIService::new(ai_config);
    let response = ai_service.chat_with_image_base64(&system_prompt, &image_base64, question_text, &[]).await?;

    Ok(serde_json::json!({
        "description": response,
        "petName": name,
    }))
}