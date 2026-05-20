use rusqlite::{params, Connection};
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let path = Path::new(db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL;").map_err(|e| format!("设置WAL模式失败: {}", e))?;
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&mut self) -> Result<(), String> {
        let migration_sql = include_str!("../../migrations/001_initial.sql");
        self.conn
            .execute_batch(migration_sql)
            .map_err(|e| format!("Migration failed: {}", e))?;

        // 运行 002_add_app_state.sql
        let migration_sql2 = include_str!("../../migrations/002_add_app_state.sql");
        self.conn
            .execute_batch(migration_sql2)
            .map_err(|e| format!("Migration 002 failed: {}", e))?;

        // 运行 003_add_dreams.sql
        let migration_sql3 = include_str!("../../migrations/003_add_dreams.sql");
        self.conn
            .execute_batch(migration_sql3)
            .map_err(|e| format!("Migration 003 failed: {}", e))?;

        // 运行 004_add_achievements.sql
        let migration_sql4 = include_str!("../../migrations/004_add_achievements.sql");
        self.conn
            .execute_batch(migration_sql4)
            .map_err(|e| format!("Migration 004 failed: {}", e))?;

        // 运行 005_add_diary.sql
        let migration_sql5 = include_str!("../../migrations/005_add_diary.sql");
        self.conn
            .execute_batch(migration_sql5)
            .map_err(|e| format!("Migration 005 failed: {}", e))?;

        Ok(())
    }

    pub fn save_last_interaction(&self, timestamp: String) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE AppState SET LastInteraction = ?1, UpdatedAt = datetime('now') WHERE Id = 1",
                params![timestamp],
            )
            .map_err(|e| format!("Save last_interaction failed: {}", e))?;
        Ok(())
    }

    pub fn load_last_interaction(&self) -> Option<String> {
        let result = self.conn.query_row(
            "SELECT LastInteraction FROM AppState WHERE Id = 1",
            params![],
            |row| row.get(0),
        );
        match result {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    }

    pub fn save_short_term_memory(
        &self,
        id: &str,
        ghost_id: &str,
        summary: &str,
        importance: f64,
        accessibility: f64,
        sentiment: f64,
        category: Option<&str>,
        entities: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO ShortTermMemories (Id, GhostId, Summary, Importance, Accessibility, Sentiment, Category, Entities, CreatedAt, LastAccessedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))",
                params![id, ghost_id, summary, importance, accessibility, sentiment, category, entities],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_short_term_memories(
        &self,
        ghost_id: &str,
        limit: usize,
    ) -> Result<Vec<ShortTermMemoryRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Summary, Importance, Accessibility, Sentiment, Category, CreatedAt, LastAccessedAt
                 FROM ShortTermMemories WHERE GhostId = ?1 ORDER BY Importance DESC, CreatedAt DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(ShortTermMemoryRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    summary: row.get(2)?,
                    importance: row.get(3)?,
                    accessibility: row.get(4)?,
                    sentiment: row.get(5)?,
                    category: row.get(6)?,
                    created_at: row.get(7)?,
                    last_accessed_at: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn delete_short_term_memory(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM ShortTermMemories WHERE Id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn count_short_term_memories(&self, ghost_id: &str) -> Result<usize, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM ShortTermMemories WHERE GhostId = ?1",
                params![ghost_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count as usize)
    }

    pub fn save_long_term_memory(
        &self,
        id: &str,
        ghost_id: &str,
        summary: &str,
        importance: f64,
        is_core_memory: bool,
        event_type: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO LongTermMemories (Id, GhostId, Summary, Importance, IsCoreMemory, EventType, Detail, CreatedAt, LastAccessedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))",
                params![id, ghost_id, summary, importance, is_core_memory as i32, event_type, detail],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_long_term_memories(
        &self,
        ghost_id: &str,
        limit: usize,
    ) -> Result<Vec<LongTermMemoryRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Summary, Importance, IsCoreMemory, EventType, CreatedAt, LastAccessedAt
                 FROM LongTermMemories WHERE GhostId = ?1 ORDER BY Importance DESC, CreatedAt DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(LongTermMemoryRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    summary: row.get(2)?,
                    importance: row.get(3)?,
                    is_core_memory: row.get::<_, i32>(4)? != 0,
                    event_type: row.get(5)?,
                    created_at: row.get(6)?,
                    last_accessed_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn get_core_memories(&self, ghost_id: &str) -> Result<Vec<LongTermMemoryRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Summary, Importance, IsCoreMemory, EventType, CreatedAt, LastAccessedAt
                 FROM LongTermMemories WHERE GhostId = ?1 AND IsCoreMemory = 1 ORDER BY Importance DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id], |row| {
                Ok(LongTermMemoryRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    summary: row.get(2)?,
                    importance: row.get(3)?,
                    is_core_memory: row.get::<_, i32>(4)? != 0,
                    event_type: row.get(5)?,
                    created_at: row.get(6)?,
                    last_accessed_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn save_experience(
        &self,
        id: &str,
        ghost_id: &str,
        name: &str,
        summary: &str,
        source: &str,
        proficiency: f64,
        source_memory_id: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO Experiences (Id, GhostId, Name, Summary, Source, Proficiency, LastUsedAt, SourceMemoryId)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), ?7)",
                params![id, ghost_id, name, summary, source, proficiency, source_memory_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_experiences(&self, ghost_id: &str) -> Result<Vec<ExperienceRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Name, Summary, Source, Proficiency, LastUsedAt, SourceMemoryId FROM Experiences WHERE GhostId = ?1 ORDER BY Proficiency DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id], |row| {
                Ok(ExperienceRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    name: row.get(2)?,
                    summary: row.get(3)?,
                    source: row.get(4)?,
                    proficiency: row.get(5)?,
                    last_used_at: row.get(6)?,
                    source_memory_id: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn update_experience_proficiency(&self, id: &str, proficiency: f64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE Experiences SET Proficiency = ?1, LastUsedAt = datetime('now') WHERE Id = ?2",
                params![proficiency, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_stm_accessibility(&self, id: &str, accessibility: f64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE ShortTermMemories SET Accessibility = ?1, LastAccessedAt = datetime('now') WHERE Id = ?2",
                params![accessibility, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn touch_memory_accessed(&self, stm_ids: &[&str], ltm_ids: &[&str]) -> Result<(), String> {
        for id in stm_ids {
            self.conn
                .execute(
                    "UPDATE ShortTermMemories SET LastAccessedAt = datetime('now') WHERE Id = ?1",
                    params![id],
                )
                .map_err(|e| e.to_string())?;
        }
        for id in ltm_ids {
            self.conn
                .execute(
                    "UPDATE LongTermMemories SET LastAccessedAt = datetime('now') WHERE Id = ?1",
                    params![id],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn save_conversation_chunk(
        &self,
        id: &str,
        ghost_id: &str,
        summary: &str,
        importance: f64,
        token_count: Option<i32>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO ConversationChunks (Id, GhostId, StartTime, Summary, Importance, TokenCount) VALUES (?1, ?2, datetime('now'), ?3, ?4, ?5)",
                params![id, ghost_id, summary, importance, token_count],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_conversation_chunks(
        &self,
        ghost_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationChunkRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, StartTime, EndTime, Summary, Importance, TokenCount FROM ConversationChunks WHERE GhostId = ?1 ORDER BY StartTime DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(ConversationChunkRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    start_time: row.get(2)?,
                    end_time: row.get(3)?,
                    summary: row.get(4)?,
                    importance: row.get(5)?,
                    token_count: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn close_conversation_chunk(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE ConversationChunks SET EndTime = datetime('now') WHERE Id = ?1",
                params![id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_impression(
        &self,
        ghost_id: &str,
        openness: f64,
        conscientiousness: f64,
        extraversion: f64,
        agreeableness: f64,
        neuroticism: f64,
        creativity: f64,
        affinity: f64,
        snippets: &str,
    ) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO MasterImpressions (Id, GhostId, OpennessScore, ConscientiousnessScore, ExtraversionScore, AgreeablenessScore, NeuroticismScore, CreativityScore, OverallAffinity, Snippets, UpdatedAt)
                 VALUES (
                    COALESCE((SELECT Id FROM MasterImpressions WHERE GhostId = ?1), ?10),
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
                params![ghost_id, openness, conscientiousness, extraversion, agreeableness, neuroticism, creativity, affinity, snippets, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_emotional_event(
        &self,
        id: &str,
        ghost_id: &str,
        event_type: &str,
        intensity: f64,
        love_hate_delta: f64,
        baseline_delta: f64,
        description: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO EmotionalEvents (Id, GhostId, EventType, Intensity, LoveHateDelta, BaselineDelta, Description, CreatedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![id, ghost_id, event_type, intensity, love_hate_delta, baseline_delta, description],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_chat_message(
        &self,
        id: &str,
        ghost_id: &str,
        role: &str,
        content: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO ChatMessages (Id, GhostId, Role, Content, CreatedAt) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                params![id, ghost_id, role, content],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load_chat_history(
        &self,
        ghost_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessageRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Role, Content, CreatedAt FROM ChatMessages WHERE GhostId = ?1 ORDER BY CreatedAt DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let mut rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(ChatMessageRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        rows.reverse();
        Ok(rows)
    }

    pub fn load_emotional_events(
        &self,
        ghost_id: &str,
        limit: usize,
    ) -> Result<Vec<EmotionalEventRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, EventType, Intensity, LoveHateDelta, BaselineDelta, Description, CreatedAt
                 FROM EmotionalEvents WHERE GhostId = ?1 ORDER BY CreatedAt DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(EmotionalEventRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    event_type: row.get(2)?,
                    intensity: row.get(3)?,
                    love_hate_delta: row.get(4)?,
                    baseline_delta: row.get(5)?,
                    description: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn load_impression(&self, ghost_id: &str) -> Option<ImpressionRow> {
        let result = self.conn
            .query_row(
                "SELECT OpennessScore, ConscientiousnessScore, ExtraversionScore, AgreeablenessScore, NeuroticismScore, CreativityScore, OverallAffinity, Snippets
                 FROM MasterImpressions WHERE GhostId = ?1",
                params![ghost_id],
                |row| {
                    Ok(ImpressionRow {
                        openness_score: row.get(0)?,
                        conscientiousness_score: row.get(1)?,
                        extraversion_score: row.get(2)?,
                        agreeableness_score: row.get(3)?,
                        neuroticism_score: row.get(4)?,
                        creativity_score: row.get(5)?,
                        overall_affinity: row.get(6)?,
                        snippets: row.get(7)?,
                    })
                },
            );
        match result {
            Ok(r) => Some(r),
            Err(_) => None,
        }
    }

    pub fn blur_core_memories_for_transfer(&self, ghost_id: &str, generation: u32) -> Result<usize, String> {
        let memories = self.get_long_term_memories(ghost_id, 1000)?;
        let mut blurred_count = 0;
        let blur_amount = 0.05 * generation as f64;

        for m in &memories {
            if m.is_core_memory && m.importance > 0.7 {
                let new_importance = (m.importance - blur_amount).max(0.7);
                self.conn
                    .execute(
                        "UPDATE LongTermMemories SET Importance = ?1, IsBlurred = 1 WHERE Id = ?2",
                        params![new_importance, m.id],
                    )
                    .map_err(|e| e.to_string())?;
                blurred_count += 1;
            }
        }
        Ok(blurred_count)
    }

    pub fn update_long_term_memory_summary(&self, id: &str, new_summary: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE LongTermMemories SET Summary = ?1 WHERE Id = ?2",
                params![new_summary, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_short_term_memories(&self, ghost_id: &str) -> Result<usize, String> {
        let count = self.count_short_term_memories(ghost_id)?;
        self.conn
            .execute("DELETE FROM ShortTermMemories WHERE GhostId = ?1", params![ghost_id])
            .map_err(|e| e.to_string())?;
        Ok(count)
    }

    pub fn clear_chat_history(&self, ghost_id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM ChatMessages WHERE GhostId = ?1", params![ghost_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_dream(
        &self,
        id: &str,
        ghost_id: &str,
        summary: &str,
        memory_snippet: Option<&str>,
        dream_type: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO Dreams (Id, GhostId, Summary, MemorySnippet, DreamType, CreatedAt) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![id, ghost_id, summary, memory_snippet, dream_type],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_recent_dreams(&self, ghost_id: &str, limit: usize) -> Result<Vec<DreamRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Summary, MemorySnippet, DreamType, CreatedAt FROM Dreams WHERE GhostId = ?1 ORDER BY CreatedAt DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(DreamRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    summary: row.get(2)?,
                    memory_snippet: row.get(3)?,
                    dream_type: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn save_achievement(&self, id: &str, ghost_id: &str, key: &str) -> Result<bool, String> {
        let rows = self.conn.execute(
            "INSERT OR IGNORE INTO Achievements (Id, GhostId, AchievementKey) VALUES (?1, ?2, ?3)",
            params![id, ghost_id, key],
        ).map_err(|e| e.to_string())?;
        Ok(rows > 0)
    }

    pub fn get_achievements(&self, ghost_id: &str) -> Result<Vec<AchievementRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, AchievementKey, UnlockedAt FROM Achievements WHERE GhostId = ?1 ORDER BY UnlockedAt ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id], |row| {
                Ok(AchievementRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    achievement_key: row.get(2)?,
                    unlocked_at: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn count_chat_messages(&self, ghost_id: &str) -> Result<usize, String> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ChatMessages WHERE GhostId = ?1",
            params![ghost_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        Ok(count as usize)
    }

    pub fn count_emotional_events(&self, ghost_id: &str) -> Result<usize, String> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM EmotionalEvents WHERE GhostId = ?1",
            params![ghost_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        Ok(count as usize)
    }

    pub fn count_dreams(&self, ghost_id: &str) -> Result<usize, String> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM Dreams WHERE GhostId = ?1",
            params![ghost_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        Ok(count as usize)
    }

    pub fn save_diary(&self, id: &str, ghost_id: &str, summary: &str, entry_date: &str) -> Result<(), String> {
        self.conn.execute(
            "INSERT OR REPLACE INTO Diary (Id, GhostId, Summary, EntryDate) VALUES (?1, ?2, ?3, ?4)",
            params![id, ghost_id, summary, entry_date],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_diary_entries(&self, ghost_id: &str, limit: usize) -> Result<Vec<DiaryRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, GhostId, Summary, EntryDate, CreatedAt FROM Diary WHERE GhostId = ?1 ORDER BY EntryDate DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![ghost_id, limit], |row| {
                Ok(DiaryRow {
                    id: row.get(0)?,
                    ghost_id: row.get(1)?,
                    summary: row.get(2)?,
                    entry_date: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }
}

#[derive(Debug)]
pub struct ShortTermMemoryRow {
    pub id: String,
    pub ghost_id: String,
    pub summary: String,
    pub importance: f64,
    pub accessibility: f64,
    pub sentiment: f64,
    pub category: Option<String>,
    pub created_at: String,
    pub last_accessed_at: Option<String>,
}

#[derive(Debug)]
pub struct LongTermMemoryRow {
    pub id: String,
    pub ghost_id: String,
    pub summary: String,
    pub importance: f64,
    pub is_core_memory: bool,
    pub event_type: Option<String>,
    pub created_at: String,
    pub last_accessed_at: Option<String>,
}

#[derive(Debug)]
pub struct ExperienceRow {
    pub id: String,
    pub ghost_id: String,
    pub name: String,
    pub summary: String,
    pub source: String,
    pub proficiency: f64,
    pub last_used_at: Option<String>,
    pub source_memory_id: Option<String>,
}

#[derive(Debug)]
pub struct EmotionalEventRow {
    pub id: String,
    pub ghost_id: String,
    pub event_type: String,
    pub intensity: f64,
    pub love_hate_delta: f64,
    pub baseline_delta: f64,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug)]
pub struct ChatMessageRow {
    pub id: String,
    pub ghost_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct ImpressionRow {
    pub openness_score: f64,
    pub conscientiousness_score: f64,
    pub extraversion_score: f64,
    pub agreeableness_score: f64,
    pub neuroticism_score: f64,
    pub creativity_score: f64,
    pub overall_affinity: f64,
    pub snippets: Option<String>,
}

#[derive(Debug)]
pub struct ConversationChunkRow {
    pub id: String,
    pub ghost_id: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub summary: String,
    pub importance: f64,
    pub token_count: Option<i32>,
}

#[derive(Debug)]
pub struct DreamRow {
    pub id: String,
    pub ghost_id: String,
    pub summary: String,
    pub memory_snippet: Option<String>,
    pub dream_type: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct AchievementRow {
    pub id: String,
    pub ghost_id: String,
    pub achievement_key: String,
    pub unlocked_at: String,
}

#[derive(Debug)]
pub struct DiaryRow {
    pub id: String,
    pub ghost_id: String,
    pub summary: String,
    pub entry_date: String,
    pub created_at: String,
}
