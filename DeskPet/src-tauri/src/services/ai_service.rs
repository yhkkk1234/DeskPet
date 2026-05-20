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
            .map_err(|e| eprintln!("[AI] 创建HTTP客户端失败，使用默认: {}", e))
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

        let url = format!("{}/chat/completions", self.config.endpoint.trim_end_matches('/'));

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

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!(
                "AI 响应格式异常，未找到 content 字段\n响应: {}",
                truncate_str(&response_text, 500)
            ))
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

        let url = format!("{}/chat/completions", self.config.endpoint.trim_end_matches('/'));

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

                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" {
                            return Ok(full_response);
                        }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                on_token(content);
                                full_response.push_str(content);
                            }
                        }
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

        let url = format!("{}/chat/completions", self.config.endpoint.trim_end_matches('/'));

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

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!(
                "AI 视觉响应格式异常\n响应: {}",
                truncate_str(&response_text, 500)
            ))
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

        let url = format!("{}/images/generations", endpoint.trim_end_matches('/'));

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

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}