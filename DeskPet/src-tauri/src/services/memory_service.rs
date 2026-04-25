use crate::data::database::Database;
use crate::core::memory::experience::Experience;
use crate::core::prompt::token_budget::MemoryPriority;

const STM_COMPRESS_THRESHOLD: usize = 20;

struct PrioritizedMemory {
    summary: String,
    priority: MemoryPriority,
    importance: f64,
}

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
        let mut all_memories: Vec<PrioritizedMemory> = Vec::new();
        let mut used_ids = std::collections::HashSet::new();

        let mut stm_ids: Vec<String> = Vec::new();
        let mut ltm_ids: Vec<String> = Vec::new();

        let core = self.db.get_core_memories(&self.ghost_id)?;
        for m in &core {
            all_memories.push(PrioritizedMemory {
                summary: m.summary.clone(),
                priority: MemoryPriority::CoreLongTerm,
                importance: m.importance,
            });
            used_ids.insert(m.id.clone());
            ltm_ids.push(m.id.clone());
        }

        let recent_stm = self.db.get_short_term_memories(&self.ghost_id, 3)?;
        for m in &recent_stm {
            if used_ids.contains(&m.id) { continue; }
            all_memories.push(PrioritizedMemory {
                summary: m.summary.clone(),
                priority: MemoryPriority::RecentConversation,
                importance: m.importance,
            });
            used_ids.insert(m.id.clone());
            stm_ids.push(m.id.clone());
        }

        let ltm = self.db.get_long_term_memories(&self.ghost_id, 50)?;
        for m in &ltm {
            if used_ids.contains(&m.id) { continue; }
            let priority = MemoryPriority::from_importance_and_type(
                m.importance, m.is_core_memory, false, 0.5,
            );
            all_memories.push(PrioritizedMemory {
                summary: m.summary.clone(),
                priority,
                importance: m.importance,
            });
            used_ids.insert(m.id.clone());
            ltm_ids.push(m.id.clone());
        }

        let stm = self.db.get_short_term_memories(&self.ghost_id, 30)?;
        for m in &stm {
            if used_ids.contains(&m.id) { continue; }
            let priority = MemoryPriority::from_importance_and_type(
                m.importance, false, true, m.accessibility,
            );
            all_memories.push(PrioritizedMemory {
                summary: m.summary.clone(),
                priority,
                importance: m.importance,
            });
            used_ids.insert(m.id.clone());
            stm_ids.push(m.id.clone());
        }

        all_memories.sort_by(|a, b| {
            (a.priority as u8).cmp(&(b.priority as u8))
                .then_with(|| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal))
        });

        let stm_refs: Vec<&str> = stm_ids.iter().map(|s| s.as_str()).collect();
        let ltm_refs: Vec<&str> = ltm_ids.iter().map(|s| s.as_str()).collect();
        let _ = self.db.touch_memory_accessed(&stm_refs, &ltm_refs);

        let mut result = Vec::new();
        for m in &all_memories {
            if result.len() >= budget { break; }
            if m.priority == MemoryPriority::ShortTermLowAccessibility {
                let mut rng = rand::thread_rng();
                if rand::Rng::gen::<f64>(&mut rng) >= 0.3 {
                    continue;
                }
            }
            let label = match m.priority {
                MemoryPriority::CoreLongTerm => "[核心记忆]",
                MemoryPriority::RecentConversation => "[最近]",
                MemoryPriority::HighImportanceLongTerm => "[记忆]",
                MemoryPriority::ShortTermHighAccessibility | MemoryPriority::ShortTermLowAccessibility => "[短期]",
                _ => "[记忆]",
            };
            result.push(format!("{} {}", label, m.summary));
        }

        Ok(result)
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
                last_used_at: r.last_used_at
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now),
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

    pub fn decay_experience_proficiency(&self) -> Result<usize, String> {
        let experiences = self.get_experiences()?;
        let mut decayed = 0;
        let now = chrono::Utc::now();
        for mut exp in experiences {
            let days = (now - exp.last_used_at).num_days().max(0) as f64;
            let old_p = exp.proficiency;
            exp.decay_proficiency(days);
            if (exp.proficiency - old_p).abs() > f64::EPSILON {
                self.db.update_experience_proficiency(&exp.id, exp.proficiency)?;
                decayed += 1;
            }
        }
        Ok(decayed)
    }
}
