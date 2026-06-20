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

/// 送给 AI 摘要的最大字符数。超过此长度的文档会被截断，
/// 避免一次性打爆 token 上限或触发 API 的输入长度限制。
const MAX_SUMMARY_CHARS: usize = 50000;

// 架构备忘（未来重构方向，当前不必处理）：
// 现在的超长文档是「单次截断前 N 字」，会丢失后文细节。
// 更成熟的方案是「分块摘要 + 检索回查」双层架构：
//   1. 导入时把全文切块，每块各自 AI 摘要，再合并出全局摘要（省 token、不爆窗口）
//   2. 全文始终保留在 DocumentKnowledge.FullText
//   3. 用户聊到具体细节时，用 search_document_context 从全文检索相关片段，
//      临时补进本次对话 prompt —— 这一段是无损的，能回答分块摘要丢掉的细节
// 关键判断：光做分块压缩会丢细节，必须配「按需检索全文」兜底才有意义。
// 本项目已埋好地基（FullText 存储 + search_document_context 命令），
// 缺的只是分块摘要逻辑 + 前端接通检索入口。

pub async fn generate_summary(
    ai_config: &AIProviderConfig,
    title: &str,
    full_text: &str,
) -> Result<String, String> {
    // 超长文档真正截断，而不是只加标注仍把全文发出去。
    let (body, truncated_note) = if full_text.chars().count() > MAX_SUMMARY_CHARS {
        let head: String = full_text.chars().take(MAX_SUMMARY_CHARS).collect();
        (
            head,
            format!("（注意：原文过长，已截取前{}字，可能丢失后文内容）\n\n", MAX_SUMMARY_CHARS),
        )
    } else {
        (full_text.to_string(), String::new())
    };

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
                "请分析以下作品的全文内容，并返回结构化的摘要JSON：\n\n{}{}\n\n---\n请只返回JSON，不要其他内容。",
                truncated_note, body
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
