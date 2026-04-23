use serde::{Deserialize, Serialize};

use super::memory_base::MemoryBase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermMemory {
    pub base: MemoryBase,
    pub detail: Option<String>,
    pub is_core_memory: bool,
    pub is_blurred: bool,
    pub event_type: Option<String>,
}

impl LongTermMemory {
    pub fn new(ghost_id: &str, summary: String, importance: f64, is_core_memory: bool) -> Self {
        Self {
            base: MemoryBase::new(ghost_id, summary, importance),
            detail: None,
            is_core_memory,
            is_blurred: false,
            event_type: None,
        }
    }

    pub fn from_short_term(stm: &super::short_term_memory::ShortTermMemory, is_core: bool) -> Self {
        Self {
            base: MemoryBase {
                id: stm.base.id.clone(),
                ghost_id: stm.base.ghost_id.clone(),
                created_at: stm.base.created_at,
                last_accessed_at: stm.base.last_accessed_at,
                importance: stm.base.importance,
                summary: stm.base.summary.clone(),
                is_compressed: true,
            },
            detail: None,
            is_core_memory: is_core,
            is_blurred: false,
            event_type: stm.category.clone(),
        }
    }

    pub fn should_never_forget(&self) -> bool {
        self.is_core_memory && self.base.importance >= 0.8
    }

    pub fn blur_for_transfer(&mut self, generation: u32) {
        if !self.is_core_memory {
            return;
        }
        let _blur_factor = 0.1 * generation as f64;
        if self.base.importance > 0.7 {
            self.base.importance = (self.base.importance - 0.05 * generation as f64).max(0.7);
        }
        self.is_blurred = true;
    }
}