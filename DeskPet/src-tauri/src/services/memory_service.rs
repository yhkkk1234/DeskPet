use crate::data::database::Database;
use crate::core::memory::experience::Experience;

const STM_COMPRESS_THRESHOLD: usize = 20;

pub struct MemoryService<'a> {
    db: &'a Database,
    ghost_id: String,
}

impl<'a> MemoryService<'a> {
    pub fn new(db: &'a Database, ghost_id: &str) -> Self {
        Self {
            db,
            ghost_id: ghost_id.to_string(),
        }
    }

    pub fn add_short_term_memory(
        &self,
        summary: &str,
        importance: f64,
        accessibility: f64,
        sentiment: f64,
        category: Option<&str>,
        entities: Option<&str>,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.db.save_short_term_memory(
            &id,
            &self.ghost_id,
            summary,
            importance,
            accessibility,
            sentiment,
            category,
            entities,
        )?;
        Ok(id)
    }

    pub fn should_compress(&self) -> Result<bool, String> {
        let count = self.db.count_short_term_memories(&self.ghost_id)?;
        Ok(count >= STM_COMPRESS_THRESHOLD)
    }

    pub fn get_memories_for_context(&self, budget: usize) -> Result<Vec<String>, String> {
        let mut memories = Vec::new();

        let core = self.db.get_core_memories(&self.ghost_id)?;
        for m in &core {
            memories.push(format!("[核心记忆] {}", m.summary));
        }

        let recent = self.db.get_short_term_memories(&self.ghost_id, 3)?;
        for m in &recent {
            memories.push(format!("[最近] {}", m.summary));
        }

        let remaining = budget.saturating_sub(memories.len());
        if remaining > 0 {
            let ltm = self.db.get_long_term_memories(&self.ghost_id, remaining)?;
            for m in &ltm {
                if !memories.len() >= budget {
                    break;
                }
                memories.push(format!("[记忆] {}", m.summary));
            }
        }

        let stm = self.db.get_short_term_memories(&self.ghost_id, 10)?;
        for m in &stm {
            if m.accessibility < 0.3 {
                let mut rng = rand::thread_rng();
                if rand::Rng::gen::<f64>(&mut rng) >= m.accessibility + 0.1 {
                    continue;
                }
            }
            memories.push(format!("[短期] {}", m.summary));
        }

        Ok(memories)
    }

    pub fn get_experiences(&self) -> Result<Vec<Experience>, String> {
        let rows = self.db.get_experiences(&self.ghost_id)?;
        Ok(rows
            .into_iter()
            .map(|r| Experience {
                id: r.id,
                ghost_id: r.ghost_id,
                name: r.name,
                summary: r.summary,
                source: r.source,
                proficiency: r.proficiency,
                last_used_at: chrono::Utc::now(),
                source_memory_id: None,
            })
            .collect())
    }

    pub fn promote_to_long_term(
        &self,
        stm_id: &str,
        summary: &str,
        importance: f64,
        is_core: bool,
        event_type: Option<&str>,
    ) -> Result<String, String> {
        let ltm_id = uuid::Uuid::new_v4().to_string();
        self.db.save_long_term_memory(
            &ltm_id,
            &self.ghost_id,
            summary,
            importance,
            is_core,
            event_type,
            None,
        )?;
        self.db.delete_short_term_memory(stm_id)?;
        Ok(ltm_id)
    }

    pub fn save_experience(
        &self,
        name: &str,
        summary: &str,
        source: &str,
        proficiency: f64,
        source_memory_id: Option<&str>,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.db.save_experience(
            &id,
            &self.ghost_id,
            name,
            summary,
            source,
            proficiency,
            source_memory_id,
        )?;
        Ok(id)
    }
}