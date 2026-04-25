use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}

pub struct ContextWindow {
    messages: Vec<ChatMessage>,
    max_tokens: usize,
    current_tokens: usize,
}

impl ContextWindow {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_tokens,
            current_tokens: 0,
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        let estimated_tokens = estimate_tokens(content);
        let msg = ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.current_tokens += estimated_tokens;
        self.messages.push(msg);
    }

    pub fn should_compress(&self) -> bool {
        self.current_tokens > self.max_tokens
    }

    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn get_recent(&self, count: usize) -> &[ChatMessage] {
        let start = self.messages.len().saturating_sub(count);
        &self.messages[start..]
    }

    pub fn clear_old_messages(&mut self, keep_count: usize) -> Vec<ChatMessage> {
        if self.messages.len() <= keep_count {
            return Vec::new();
        }
        let remove_count = self.messages.len() - keep_count;
        let removed: Vec<ChatMessage> = self.messages.drain(..remove_count).collect();
        for msg in &removed {
            self.current_tokens = self.current_tokens.saturating_sub(estimate_tokens(&msg.content));
        }
        removed
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.current_tokens = 0;
    }

    pub fn token_count(&self) -> usize {
        self.current_tokens
    }
}

/// 混合字符估算法：CJK字符≈1.5 tokens，ASCII字符≈0.3 tokens
/// 比简单的 bytes/2 对中文文本更准确
fn estimate_tokens(text: &str) -> usize {
    let mut tokens: f64 = 0.0;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            tokens += 0.3;
        } else if ch.is_ascii_whitespace() || ch.is_ascii_punctuation() {
            tokens += 0.2;
        } else {
            tokens += 1.5;
        }
    }
    tokens.ceil() as usize
}