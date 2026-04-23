use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBase {
    pub id: String,
    pub ghost_id: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub importance: f64,
    pub summary: String,
    pub is_compressed: bool,
}

impl MemoryBase {
    pub fn new(ghost_id: &str, summary: String, importance: f64) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            ghost_id: ghost_id.to_string(),
            created_at: now,
            last_accessed_at: now,
            importance: importance.clamp(0.0, 1.0),
            summary,
            is_compressed: false,
        }
    }
}