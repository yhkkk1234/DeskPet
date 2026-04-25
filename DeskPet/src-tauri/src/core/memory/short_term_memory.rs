use serde::{Deserialize, Serialize};
use super::memory_base::MemoryBase;
use super::local_compressor;
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

    pub fn new_with_compress(ghost_id: &str, raw_message: &str, importance: f64, accessibility: f64, category: Option<&str>) -> Self {
        let compressed = local_compressor::local_compress_message(raw_message);
        Self {
            base: MemoryBase::new(ghost_id, compressed.summary, importance),
            accessibility: accessibility.clamp(0.0, 1.0),
            category: category.map(|s| s.to_string()),
            entities: compressed.entities,
            sentiment: compressed.sentiment,
        }
    }

    pub fn should_include_in_context(&self) -> bool {
        if self.accessibility >= 0.3 {
            return true;
        }
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.accessibility + 0.1
    }

    /// 使用本地即时压缩器更新摘要、实体和情感
    pub fn local_compress(&mut self) {
        let compressed = local_compressor::local_compress_message(&self.base.summary);
        self.base.summary = compressed.summary;
        self.entities = compressed.entities;
        self.sentiment = compressed.sentiment;
        self.base.is_compressed = true;
    }
}