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
        let estimated_tokens = content.len() / 2;
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
            self.current_tokens = self.current_tokens.saturating_sub(msg.content.len() / 2);
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