use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub id: String,
    pub ghost_id: String,
    pub name: String,
    pub summary: String,
    pub source: String,
    pub proficiency: f64,
    pub last_used_at: DateTime<Utc>,
    pub source_memory_id: Option<String>,
}

impl Experience {
    pub fn new(ghost_id: &str, name: String, summary: String, source: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            ghost_id: ghost_id.to_string(),
            name,
            summary,
            source,
            proficiency: 0.3,
            last_used_at: Utc::now(),
            source_memory_id: None,
        }
    }

    pub fn use_experience(&mut self) {
        self.proficiency = (self.proficiency + 0.01).min(1.0);
        self.last_used_at = Utc::now();
    }

    pub fn decay_proficiency(&mut self, days_since_last_use: f64) {
        let decay = 0.001 * days_since_last_use;
        self.proficiency = (self.proficiency - decay).max(0.1);
    }

    pub fn to_prompt_instruction(&self) -> String {
        let level = match self.proficiency {
            p if p > 0.8 => "很擅长",
            p if p > 0.6 => "比较擅长",
            p if p > 0.4 => "有一定了解",
            p if p > 0.2 => "略知一二",
            _ => "只是听说过",
        };
        format!("{}（{}）：{}", self.name, level, self.summary)
    }
}