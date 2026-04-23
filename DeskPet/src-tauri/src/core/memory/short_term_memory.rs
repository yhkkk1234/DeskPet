use serde::{Deserialize, Serialize};
use super::memory_base::MemoryBase;
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermMemory {
    pub base: MemoryBase,
    pub accessibility: f64,
    pub category: Option<String>,
    pub entities: Vec<String>,
    pub sentiment: f64,
}

impl ShortTermMemory {
    pub fn new(ghost_id: &str, summary: String, importance: f64, accessibility: f64) -> Self {
        Self {
            base: MemoryBase::new(ghost_id, summary, importance),
            accessibility: accessibility.clamp(0.0, 1.0),
            category: None,
            entities: Vec::new(),
            sentiment: 0.0,
        }
    }

    pub fn should_include_in_context(&self) -> bool {
        if self.accessibility >= 0.3 {
            return true;
        }
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.accessibility + 0.1
    }

    pub fn local_compress(&mut self) {
        if self.base.summary.len() > 100 {
            let truncated: String = self.base.summary.chars().take(100).collect();
            self.base.summary = format!("{}...", truncated);
        }
        self.base.is_compressed = true;
    }
}