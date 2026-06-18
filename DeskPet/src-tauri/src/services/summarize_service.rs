use crate::services::ai_service::{AIService, AIProviderConfig, ChatMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub one_liner: String,
    pub genres: Vec<String>,
    pub characters: Vec<CharacterInfo>,
    pub plot_arc: String,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub name: String,
    pub role: String,
    pub traits: String,
}

pub async fn generate_summary(
    ai_config: &AIProviderConfig,
    title: &str,
    full_text: &str,
) -> Result<String, String> {
    let system_prompt = format!(
        r#"你是一个文学分析助手。请阅读以下作品全文，生成一份结构化的摘要。

要求：
1. 用中文回复
2. 只返回 JSON，不要任何其他文字
3. JSON 格式如下：
{{
  "one_liner": "一句话概括故事内容（50字以内）",
  "genres": ["题材标签1", "题材标签2"],
  "characters": [
    {{ "name": "角色名", "role": "身份/定位", "traits": "核心性格特征（20字以内）" }}
  ],
  "plot_arc": "主要情节发展脉络（200字以内）",
  "themes": ["核心主题1", "核心主题2"]
}}

作品标题：《{}》"#,
        title
    );

    let messages = vec![
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "请分析以下作品的全文内容，并返回结构化的摘要JSON：\n\n{}\n\n---\n请只返回JSON，不要其他内容。",
                if full_text.chars().count() > 50000 {
                    format!("（以下为原文，共{}字）\n{}", crate::services::extract_service::count_chinese_words(full_text), full_text)
                } else {
                    full_text.to_string()
                }
            ),
        },
    ];

    let ai_service = AIService::new(ai_config.clone());
    let response = ai_service.chat(&system_prompt, &messages).await?;

    let json_str = extract_json_from_response(&response);
    serde_json::from_str::<DocumentSummary>(&json_str)
        .map_err(|e| format!("摘要JSON解析失败: {}，原始响应: {}", e, truncate_str(&response, 300)))?;

    Ok(json_str)
}

fn extract_json_from_response(response: &str) -> String {
    let trimmed = response.trim();

    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}
