use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProviderConfig {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub vision_model: Option<String>,
    pub image_model: Option<String>,
    pub image_gen_endpoint: Option<String>,
    pub image_gen_api_key: Option<String>,
    pub is_default: bool,
    // 语音转写（STT）配置：OpenAI 兼容 /audio/transcriptions 端点。
    // 默认跟随主 endpoint/key；留空表示未启用语音输入。
    #[serde(default)]
    pub stt_endpoint: Option<String>,
    #[serde(default)]
    pub stt_api_key: Option<String>,
    #[serde(default)]
    pub stt_model: Option<String>,
}

pub struct AIService {
    config: AIProviderConfig,
    client: reqwest::Client,
}

impl AIService {
    pub fn new(config: AIProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| tracing::error!("[AI] 创建HTTP客户端失败，使用默认: {}", e))
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    pub async fn chat(&self, system_prompt: &str, messages: &[ChatMessage]) -> Result<String, String> {
        if self.config.api_key.trim().is_empty() {
            return Err("API Key 为空，请在「配置 AI 接口」中填写您的 API Key".into());
        }

        let mut body = serde_json::json!({
            "model": self.config.model,
            "messages": [],
        });

        body["messages"].as_array_mut().unwrap().push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));

        for msg in messages {
            body["messages"].as_array_mut().unwrap().push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }

        let url = build_chat_url(&self.config.endpoint);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("网络请求失败: {} (URL: {})", e, url))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("读取响应失败: {}", e))?;

        if !status.is_success() {
            let error_detail = extract_error_message(&response_text);
            return Err(format!(
                "AI API 错误 [{}]: {} (URL: {})",
                status.as_u16(),
                error_detail,
                url
            ));
        }

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("解析JSON失败: {}\n原始响应: {}", e, truncate_str(&response_text, 500)))?;

        // 兼容思考型模型：优先 message.content，为空时 fallback 到 message.reasoning_content
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| response_json["choices"][0]["message"]["reasoning_content"].as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!(
                "AI 响应格式异常，未找到 content 字段\n响应: {}",
                truncate_str(&response_text, 500)
            ))?;
        Ok(content)
    }

    pub async fn chat_streaming<F>(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        on_token: F,
    ) -> Result<String, String>
    where
        F: Fn(&str),
    {
        if self.config.api_key.trim().is_empty() {
            return Err("API Key 为空，请在「配置 AI 接口」中填写您的 API Key".into());
        }

        let mut body = serde_json::json!({
            "model": self.config.model,
            "messages": [],
            "stream": true,
        });

        body["messages"].as_array_mut().unwrap().push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));

        for msg in messages {
            body["messages"].as_array_mut().unwrap().push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }

        let url = build_chat_url(&self.config.endpoint);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("流式请求失败: {} (URL: {})", e, url))?;

        let status = response.status();
        if !status.is_success() {
            let response_text = response.text().await
                .map_err(|e| format!("读取响应失败: {}", e))?;
            let error_detail = extract_error_message(&response_text);
            return Err(format!(
                "AI API 流式错误 [{}]: {} (URL: {})",
                status.as_u16(),
                error_detail,
                url
            ));
        }

        let mut stream = response.bytes_stream();
        let mut full_response = String::new();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| format!("读取流失败: {}", e))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            loop {
                if let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    // 跳过空行和 SSE 注释行（以 : 开头，如 :keep-alive / : ping）
                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    // 放宽 data: 前缀匹配：容忍 "data: " 和 "data:" 两种格式
                    let data = if let Some(rest) = line.strip_prefix("data: ") {
                        rest
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        rest.trim_start()
                    } else {
                        // 非 data 行，跳过（如 event:、id: 等其他 SSE 字段）
                        continue;
                    };

                    if data == "[DONE]" {
                        return Ok(full_response);
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        // 兼容思考型模型：优先 delta.content，为 null/空时读 delta.reasoning_content
                        // 思考过程静默丢弃（不 emit 给前端），只攒最终回答到 full_response
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                            if !content.is_empty() {
                                on_token(content);
                                full_response.push_str(content);
                            }
                        }
                        // reasoning_content 不 emit、不攒，避免思考过程污染对话
                    }
                } else {
                    break;
                }
            }
        }

        Ok(full_response)
    }

    pub async fn chat_with_image(&self, system_prompt: &str, image_path: &str, question: &str, messages: &[ChatMessage]) -> Result<String, String> {
        let image_path = image_path.to_string();
        let image_data = tokio::task::spawn_blocking(move || {
            std::fs::read(&image_path).map_err(|e| format!("读取图片失败: {}", e))
        }).await.map_err(|e| format!("读取图片任务失败: {}", e))??;
        let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);
        self.chat_with_image_base64(system_prompt, &base64_image, question, messages).await
    }

    pub async fn chat_with_image_base64(&self, system_prompt: &str, image_base64: &str, question: &str, messages: &[ChatMessage]) -> Result<String, String> {
        if self.config.api_key.trim().is_empty() {
            return Err("API Key 为空，请先配置 AI 接口".into());
        }

        let vision_model = self.config.vision_model.clone().unwrap_or_else(|| self.config.model.clone());

        let mut body = serde_json::json!({
            "model": vision_model,
            "messages": [],
        });

        body["messages"].as_array_mut().unwrap().push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));

        for msg in messages {
            body["messages"].as_array_mut().unwrap().push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }

        body["messages"].as_array_mut().unwrap().push(serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": question,
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", image_base64),
                    },
                },
            ],
        }));

        let url = build_chat_url(&self.config.endpoint);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("AI 视觉请求失败: {} (URL: {})", e, url))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("读取响应失败: {}", e))?;

        if !status.is_success() {
            let error_detail = extract_error_message(&response_text);
            return Err(format!(
                "AI 视觉 API 错误 [{}]: {} (URL: {})",
                status.as_u16(),
                error_detail,
                url
            ));
        }

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("解析JSON失败: {}\n原始响应: {}", e, truncate_str(&response_text, 500)))?;

        // 兼容思考型模型：优先 message.content，为空时 fallback 到 message.reasoning_content
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| response_json["choices"][0]["message"]["reasoning_content"].as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!(
                "AI 视觉响应格式异常\n响应: {}",
                truncate_str(&response_text, 500)
            ))?;
        Ok(content)
    }

    pub async fn generate_image(&self, prompt: &str) -> Result<String, String> {
        let image_model = self.config.image_model.clone()
            .unwrap_or_else(|| "dall-e-3".into());

        let endpoint = self.config.image_gen_endpoint.clone()
            .unwrap_or_else(|| self.config.endpoint.clone());

        let api_key = self.config.image_gen_api_key.clone()
            .unwrap_or_else(|| self.config.api_key.clone());

        if api_key.trim().is_empty() {
            return Err("API Key 为空，请先配置 AI 接口".into());
        }

        let body = serde_json::json!({
            "model": image_model,
            "prompt": prompt,
            "n": 1,
            "size": "512x512",
            "response_format": "b64_json",
        });

        let url = build_image_url(&endpoint);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("生图请求失败: {} (URL: {})", e, url))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("读取响应失败: {}", e))?;

        if !status.is_success() {
            let error_detail = extract_error_message(&response_text);
            return Err(format!(
                "生图 API 错误 [{}]: {} (URL: {})",
                status.as_u16(),
                error_detail,
                url
            ));
        }

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("解析JSON失败: {}\n原始响应: {}", e, truncate_str(&response_text, 500)))?;

        response_json["data"][0]["b64_json"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| {
                response_json["data"][0]["url"]
                    .as_str()
                    .map(|u| format!("url:{}", u))
            })
            .ok_or_else(|| format!(
                "生图响应格式异常\n响应: {}",
                truncate_str(&response_text, 500)
            ))
    }
}

fn extract_error_message(response_text: &str) -> String {
    // 检测 HTML 响应（通常是 404/502 等网关返回的错误页面，而非 JSON）
    let trimmed = response_text.trim_start();
    if trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<html") || trimmed.starts_with("<HTML") {
        return format!(
            "服务器返回了 HTML 页面而非 JSON，通常是 Endpoint 路径有误。\n\
             请检查 Endpoint 是否需要加 /v1 后缀（例如 https://api.example.com/v1），\n\
             当前拼接后会请求 /chat/completions。"
        );
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_text) {
        if let Some(error) = json.get("error") {
            if let Some(msg) = error.get("message").and_then(|m| m.as_str()) {
                return msg.to_string();
            }
            return error.to_string();
        }
    }
    truncate_str(response_text, 300)
}

/// 智能拼接 chat completions URL。
/// 规则：
/// - 去尾斜杠
/// - 已以 /chat/completions 结尾 → 直接用（用户填了完整路径）
/// - 已以 /v1、/v2 等版本号结尾 → 拼 /chat/completions
/// - 裸域名（如 https://api.deepseek.com）→ 自动补 /v1/chat/completions
/// - 路径里已含 /v1/ 等中间段 → 只拼 /chat/completions
fn build_chat_url(endpoint: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        return base.to_string();
    }
    if base.ends_with("/v1") || base.ends_with("/v2") || base.ends_with("/api") {
        return format!("{}/chat/completions", base);
    }
    if base.contains("/v1/") || base.contains("/v2/") || base.contains("/api/") {
        return format!("{}/chat/completions", base);
    }
    format!("{}/v1/chat/completions", base)
}

/// 智能拼接 images generations URL，规则同 build_chat_url 但路径为 /images/generations。
fn build_image_url(endpoint: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.ends_with("/images/generations") {
        return base.to_string();
    }
    if base.ends_with("/v1") || base.ends_with("/v2") || base.ends_with("/api") {
        return format!("{}/images/generations", base);
    }
    if base.contains("/v1/") || base.contains("/v2/") || base.contains("/api/") {
        return format!("{}/images/generations", base);
    }
    format!("{}/v1/images/generations", base)
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_chat_url_full_path() {
        // 用户填了完整路径 → 直接用
        assert_eq!(
            build_chat_url("https://api.deepseek.com/v1/chat/completions"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        // 尾部斜杠容忍
        assert_eq!(
            build_chat_url("https://api.example.com/v1/chat/completions/"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_chat_url_version_suffix() {
        assert_eq!(
            build_chat_url("https://api.deepseek.com/v1"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            build_chat_url("https://api.example.com/v2"),
            "https://api.example.com/v2/chat/completions"
        );
        assert_eq!(
            build_chat_url("https://api.example.com/api"),
            "https://api.example.com/api/chat/completions"
        );
    }

    #[test]
    fn test_build_chat_url_mid_path() {
        // 路径中间含 /v1/ → 只拼 /chat/completions
        assert_eq!(
            build_chat_url("https://example.com/v1/"),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            build_chat_url("https://example.com/api/v1"),
            "https://example.com/api/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_chat_url_bare_domain() {
        // 裸域名 → 自动补 /v1/
        assert_eq!(
            build_chat_url("https://api.deepseek.com"),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_image_url() {
        assert_eq!(
            build_image_url("https://api.example.com/v1"),
            "https://api.example.com/v1/images/generations"
        );
        assert_eq!(
            build_image_url("https://api.example.com"),
            "https://api.example.com/v1/images/generations"
        );
        assert_eq!(
            build_image_url("https://api.example.com/v1/images/generations"),
            "https://api.example.com/v1/images/generations"
        );
    }

    #[test]
    fn test_extract_error_message_html() {
        let html = "<!DOCTYPE html><html><body>404 Not Found</body></html>";
        let msg = extract_error_message(html);
        assert!(msg.contains("HTML 页面"));
        assert!(msg.contains("/v1"));
    }

    #[test]
    fn test_extract_error_message_json() {
        let json = r#"{"error":{"message":"Invalid API key","type":"auth_error"}}"#;
        assert_eq!(extract_error_message(json), "Invalid API key");
        // 无 message 字段 → 返回 error 对象原文
        let json2 = r#"{"error":{"code":429}}"#;
        assert!(extract_error_message(json2).contains("429"));
    }

    #[test]
    fn test_extract_error_message_plain_truncated() {
        let long = "x".repeat(500);
        let msg = extract_error_message(&long);
        assert!(msg.len() < 400);
        assert!(msg.ends_with("..."));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("短文本", 100), "短文本");
        let t = truncate_str("很长很长的文本", 4);
        assert!(t.ends_with("..."));
        assert!(t.chars().count() <= 8);
    }
}