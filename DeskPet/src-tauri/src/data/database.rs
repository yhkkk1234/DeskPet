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
        // rusqlite bundled 默认开启外键检查，但迁移中的 Dreams/Achievements/Diary 表
        // 外键引用了不存在的 Ghosts 表（ghost 存加密文件，不存 DB）。
        // 若保持 FK 开启，这三张表的 INSERT 会静默失败（no such table: main.Ghosts）。
        // 显式关闭，与历史行为一致。
        conn.execute_batch("PRAGMA foreign_keys=OFF;").map_err(|e| format!("关闭外键检查失败: {}", e))?;
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

        // 运行 006_add_document_knowledge.sql
        let migration_sql6 = include_str!("../../migrations/006_add_document_knowledge.sql");
        self.conn
            .execute_batch(migration_sql6)
            .map_err(|e| format!("Migration 006 failed: {}", e))?;

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
        self.save_chat_message_with_ts(id, ghost_id, role, content, None)
    }

    pub fn save_chat_message_with_ts(
        &self,
        id: &str,
        ghost_id: &str,
        role: &str,
        content: &str,
        created_at: Option<&str>,
    ) -> Result<(), String> {
        match created_at {
            Some(ts) => {
                self.conn
                    .execute(
                        "INSERT INTO ChatMessages (Id, GhostId, Role, Content, CreatedAt) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![id, ghost_id, role, content, ts],
                    )
                    .map_err(|e| e.to_string())?;
            }
            None => {
                self.conn
                    .execute(
                        "INSERT INTO ChatMessages (Id, GhostId, Role, Content, CreatedAt) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                        params![id, ghost_id, role, content],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// 删除指定 content 模式匹配的 system 消息。
    /// 用于清理重启时累积的旧欢迎消息（"灵魂已恢复"/"灵魂已注入"）。
    pub fn delete_system_messages_like(&self, pattern: &str) -> Result<usize, String> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM ChatMessages WHERE Role = 'system' AND Content LIKE ?1",
                params![pattern],
            )
            .map_err(|e| e.to_string())?;
        Ok(deleted)
    }

    /// 修复历史聊天消息的排序问题。
    ///
    /// 旧版本用 `datetime('now')`（秒精度）生成 CreatedAt，导致同一秒内保存的
    /// user/assistant 消息时间戳相同，`ORDER BY CreatedAt` 排序不稳定，
    /// 重启后加载历史时 user/assistant 顺序可能颠倒。
    ///
    /// 本方法按 CreatedAt 分组，同一秒内的消息按 role 优先级（user < assistant < system）
    /// 重新排序，并分配递增的毫秒偏移时间戳（.000/.001/.002...），确保排序稳定。
    /// 跨秒的消息天然有序，不受影响。
    ///
    /// 返回被修复的消息数量。
    pub fn repair_chat_history_order(&mut self) -> Result<usize, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT Id, GhostId, Role, Content, CreatedAt FROM ChatMessages ORDER BY CreatedAt ASC, Id ASC")
            .map_err(|e| e.to_string())?;

        let rows: Vec<ChatMessageRow> = stmt
            .query_map([], |row| {
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

        drop(stmt);

        if rows.is_empty() {
            return Ok(0);
        }

        fn role_priority(role: &str) -> i32 {
            match role {
                "user" => 0,
                "assistant" => 1,
                "system" => 2,
                _ => 3,
            }
        }

        fn is_low_precision(ts: &str) -> bool {
            !ts.contains('.')
        }

        let mut repaired: usize = 0;
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        let mut i = 0;
        while i < rows.len() {
            if !is_low_precision(&rows[i].created_at) {
                i += 1;
                continue;
            }

            let mut j = i;
            while j + 1 < rows.len()
                && is_low_precision(&rows[j + 1].created_at)
                && rows[j + 1].created_at == rows[i].created_at
            {
                j += 1;
            }

            if j > i {
                let mut group: Vec<&ChatMessageRow> = rows[i..=j].iter().collect();
                group.sort_by_key(|r| role_priority(&r.role));

                for (k, msg) in group.iter().enumerate() {
                    let new_ts = format!("{}.{}", msg.created_at.trim_end_matches(' '), k);
                    tx.execute(
                        "UPDATE ChatMessages SET CreatedAt = ?1 WHERE Id = ?2",
                        params![new_ts, msg.id],
                    )
                    .map_err(|e| e.to_string())?;
                    repaired += 1;
                }
            }

            i = j + 1;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(repaired)
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

    pub fn save_document(
        &self,
        id: &str,
        title: &str,
        file_path: &str,
        file_type: &str,
        word_count: u32,
        full_text: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO DocumentKnowledge (Id, Title, FilePath, FileType, WordCount, FullText, ImportStatus, CreatedAt, UpdatedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'summarizing', datetime('now'), datetime('now'))",
                params![id, title, file_path, file_type, word_count, full_text],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_document_summary(&self, id: &str, summary_json: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE DocumentKnowledge SET SummaryJson = ?1, ImportStatus = 'ready', UpdatedAt = datetime('now') WHERE Id = ?2",
                params![summary_json, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_document_status(&self, id: &str, status: &str, error: Option<&str>) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE DocumentKnowledge SET ImportStatus = ?1, ErrorMessage = ?2, UpdatedAt = datetime('now') WHERE Id = ?3",
                params![status, error, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_document_list(&self) -> Result<Vec<DocumentKnowledgeRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, Title, FilePath, FileType, WordCount, ImportStatus, ErrorMessage, CreatedAt, UpdatedAt
                 FROM DocumentKnowledge ORDER BY CreatedAt DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                Ok(DocumentKnowledgeRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    file_path: row.get(2)?,
                    file_type: row.get(3)?,
                    word_count: row.get(4)?,
                    import_status: row.get(5)?,
                    error_message: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    summary_json: None,
                    full_text: None,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn get_document_by_id(&self, id: &str) -> Result<DocumentKnowledgeRow, String> {
        self.conn
            .query_row(
                "SELECT Id, Title, FilePath, FileType, WordCount, SummaryJson, FullText, ImportStatus, ErrorMessage, CreatedAt, UpdatedAt
                 FROM DocumentKnowledge WHERE Id = ?1",
                params![id],
                |row| {
                    Ok(DocumentKnowledgeRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        file_path: row.get(2)?,
                        file_type: row.get(3)?,
                        word_count: row.get(4)?,
                        summary_json: row.get(5)?,
                        full_text: row.get(6)?,
                        import_status: row.get(7)?,
                        error_message: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .map_err(|e| e.to_string())
    }

    pub fn get_ready_documents(&self) -> Result<Vec<DocumentKnowledgeRow>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT Id, Title, FilePath, FileType, WordCount, SummaryJson, FullText, ImportStatus, ErrorMessage, CreatedAt, UpdatedAt
                 FROM DocumentKnowledge WHERE ImportStatus = 'ready' AND SummaryJson != '' ORDER BY CreatedAt DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                Ok(DocumentKnowledgeRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    file_path: row.get(2)?,
                    file_type: row.get(3)?,
                    word_count: row.get(4)?,
                    summary_json: row.get(5)?,
                    full_text: row.get(6)?,
                    import_status: row.get(7)?,
                    error_message: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    pub fn delete_document(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM DocumentKnowledge WHERE Id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentKnowledgeRow {
    pub id: String,
    pub title: String,
    pub file_path: String,
    pub file_type: String,
    pub word_count: i64,
    pub summary_json: Option<String>,
    pub full_text: Option<String>,
    pub import_status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::new(":memory:").expect("内存数据库初始化失败")
    }

    #[test]
    fn test_migrations_create_all_tables() {
        let db = test_db();
        let tables: Vec<String> = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for expected in [
            "ChatMessages",
            "EmotionalEvents",
            "ShortTermMemories",
            "LongTermMemories",
            "MasterImpressions",
            "ConversationChunks",
            "Experiences",
            "AppState",
            "Dreams",
            "Achievements",
            "Diary",
            "DocumentKnowledge",
        ] {
            assert!(tables.contains(&expected.to_string()), "缺少表: {}", expected);
        }
    }

    #[test]
    fn test_app_state_roundtrip() {
        let db = test_db();
        assert!(db.load_last_interaction().is_none());
        db.save_last_interaction("2026-08-03 12:00:00".into()).unwrap();
        assert_eq!(db.load_last_interaction().unwrap(), "2026-08-03 12:00:00");
    }

    #[test]
    fn test_short_term_memory_crud() {
        let db = test_db();
        db.save_short_term_memory("stm1", "g1", "低重要性记忆", 0.2, 1.0, 0.5, Some("闲聊"), Some("实体A"))
            .unwrap();
        db.save_short_term_memory("stm2", "g1", "高重要性记忆", 0.9, 0.8, -0.3, None, None)
            .unwrap();
        db.save_short_term_memory("stm3", "g2", "其他灵魂的记忆", 0.5, 0.5, 0.0, None, None)
            .unwrap();

        let rows = db.get_short_term_memories("g1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        // 按 Importance DESC 排序：高重要性在前
        assert_eq!(rows[0].id, "stm2");
        assert_eq!(rows[0].summary, "高重要性记忆");
        assert_eq!(rows[1].id, "stm1");

        assert_eq!(db.count_short_term_memories("g1").unwrap(), 2);

        db.update_stm_accessibility("stm2", 0.1).unwrap();
        let updated = db.get_short_term_memories("g1", 10).unwrap();
        assert!((updated[0].accessibility - 0.1).abs() < 1e-9);

        db.delete_short_term_memory("stm1").unwrap();
        assert_eq!(db.count_short_term_memories("g1").unwrap(), 1);

        assert_eq!(db.clear_short_term_memories("g1").unwrap(), 1);
        assert_eq!(db.count_short_term_memories("g1").unwrap(), 0);
        // g2 不受影响
        assert_eq!(db.count_short_term_memories("g2").unwrap(), 1);
    }

    #[test]
    fn test_long_term_memory_crud() {
        let db = test_db();
        db.save_long_term_memory("ltm1", "g1", "核心记忆：第一次见面", 1.0, true, Some("FirstConversation"), Some("那天阳光很好"))
            .unwrap();
        db.save_long_term_memory("ltm2", "g1", "普通记忆", 0.4, false, None, None)
            .unwrap();

        let all = db.get_long_term_memories("g1", 10).unwrap();
        assert_eq!(all.len(), 2);
        // Importance DESC：核心记忆在前
        assert_eq!(all[0].id, "ltm1");
        assert!(all[0].is_core_memory);
        assert_eq!(all[0].event_type.as_deref(), Some("FirstConversation"));

        let core = db.get_core_memories("g1").unwrap();
        assert_eq!(core.len(), 1);
        assert_eq!(core[0].id, "ltm1");

        db.update_long_term_memory_summary("ltm2", "模糊后的记忆").unwrap();
        let updated = db.get_long_term_memories("g1", 10).unwrap();
        assert_eq!(updated[1].summary, "模糊后的记忆");
    }

    #[test]
    fn test_blur_core_memories_for_transfer() {
        let db = test_db();
        db.save_long_term_memory("ltm1", "g1", "绝对重要的事", 1.0, true, None, None)
            .unwrap();
        // generation=2 → 模糊量 0.1 → importance 1.0 → 0.9（文本模糊由 AI 层完成，此处只降数值）
        let blurred = db.blur_core_memories_for_transfer("g1", 2).unwrap();
        assert_eq!(blurred, 1);
        let rows = db.get_core_memories("g1").unwrap();
        assert!((rows[0].importance - 0.9).abs() < 1e-9);
    }

    #[test]
    fn test_experience_crud() {
        let db = test_db();
        db.save_experience("exp1", "g1", "绘画技巧", "学会了用AI画图", "对话总结", 0.5, Some("ltm1"))
            .unwrap();
        db.save_experience("exp2", "g1", "编程知识", "理解了Rust所有权", "对话总结", 0.8, None)
            .unwrap();

        let rows = db.get_experiences("g1").unwrap();
        assert_eq!(rows.len(), 2);
        // Proficiency DESC
        assert_eq!(rows[0].id, "exp2");
        assert_eq!(rows[1].name, "绘画技巧");
        assert_eq!(rows[1].source_memory_id.as_deref(), Some("ltm1"));

        db.update_experience_proficiency("exp1", 0.95).unwrap();
        let rows = db.get_experiences("g1").unwrap();
        assert_eq!(rows[0].id, "exp1");
    }

    #[test]
    fn test_impression_upsert() {
        let db = test_db();
        assert!(db.load_impression("g1").is_none());

        db.save_impression("g1", 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 15.0, "片段1").unwrap();
        let row = db.load_impression("g1").unwrap();
        assert_eq!(row.openness_score, 1.0);
        assert_eq!(row.overall_affinity, 15.0);
        assert_eq!(row.snippets.as_deref(), Some("片段1"));

        // 再次保存同一 ghost 应更新而非新增（INSERT OR REPLACE + COALESCE Id）
        db.save_impression("g1", 7.0, 0.0, 0.0, 0.0, 0.0, 0.0, 30.0, "片段2").unwrap();
        let row = db.load_impression("g1").unwrap();
        assert_eq!(row.openness_score, 7.0);
        assert_eq!(row.snippets.as_deref(), Some("片段2"));
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM MasterImpressions WHERE GhostId='g1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_emotional_event_crud() {
        let db = test_db();
        db.save_emotional_event("ev1", "g1", "UserPraisedPet", 0.6, 3.0, 0.0, Some("夸奖"))
            .unwrap();
        db.save_emotional_event("ev2", "g1", "UserGotAngry", 0.5, -2.0, 0.0, None)
            .unwrap();
        db.save_emotional_event("ev3", "g2", "NormalChat", 0.3, 1.0, 0.0, None)
            .unwrap();

        assert_eq!(db.count_emotional_events("g1").unwrap(), 2);
        let rows = db.load_emotional_events("g1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        let by_id = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("ev1").event_type, "UserPraisedPet");
        assert_eq!(by_id("ev1").description.as_deref(), Some("夸奖"));
        assert_eq!(by_id("ev2").love_hate_delta, -2.0);
    }

    #[test]
    fn test_chat_message_crud() {
        let db = test_db();
        // 全部显式时间戳，避免 datetime('now') 秒精度导致同秒排序不稳定
        db.save_chat_message_with_ts("m1", "g1", "user", "你好", Some("2026-08-03 09:00:00.000"))
            .unwrap();
        db.save_chat_message_with_ts("m2", "g1", "assistant", "你好呀", Some("2026-08-03 09:00:01.000"))
            .unwrap();
        db.save_chat_message_with_ts("m3", "g1", "user", "第二条", Some("2026-08-03 09:00:02.000"))
            .unwrap();

        let rows = db.load_chat_history("g1", 10).unwrap();
        assert_eq!(rows.len(), 3);
        // 时间正序：m1 → m2 → m3
        assert_eq!(rows[0].id, "m1");
        assert_eq!(rows[1].id, "m2");
        assert_eq!(rows[2].id, "m3");

        assert_eq!(db.count_chat_messages("g1").unwrap(), 3);

        // 删除 system 消息
        db.save_chat_message("m4", "g1", "system", "灵魂已恢复").unwrap();
        let deleted = db.delete_system_messages_like("%灵魂已恢复%").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.count_chat_messages("g1").unwrap(), 3);

        db.clear_chat_history("g1").unwrap();
        assert_eq!(db.count_chat_messages("g1").unwrap(), 0);
    }

    #[test]
    fn test_repair_chat_history_order() {
        let mut db = test_db();
        // 同一秒内 user/assistant 交错保存（旧版本秒精度 bug 场景）
        db.save_chat_message_with_ts("a1", "g1", "assistant", "回复1", Some("2026-08-03 09:00:00"))
            .unwrap();
        db.save_chat_message_with_ts("u1", "g1", "user", "问题1", Some("2026-08-03 09:00:00"))
            .unwrap();
        db.save_chat_message_with_ts("a2", "g1", "assistant", "回复2", Some("2026-08-03 09:00:00"))
            .unwrap();

        let repaired = db.repair_chat_history_order().unwrap();
        assert_eq!(repaired, 3);

        let rows = db.load_chat_history("g1", 10).unwrap();
        // 修复后应为 user → assistant → assistant
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[1].role, "assistant");
        assert_eq!(rows[2].role, "assistant");
        // 时间戳带毫秒偏移（已修复为高精度）
        assert!(rows[0].created_at.contains('.'));
    }

    #[test]
    fn test_dream_crud() {
        let db = test_db();
        db.save_dream("d1", "g1", "梦见了一片草原", Some("记忆片段"), "peaceful")
            .unwrap();
        db.save_dream("d2", "g1", "梦见主人带我散步", None, "adventure").unwrap();

        assert_eq!(db.count_dreams("g1").unwrap(), 2);
        let rows = db.get_recent_dreams("g1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        let by_id = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("d1").memory_snippet.as_deref(), Some("记忆片段"));
        assert_eq!(by_id("d1").dream_type, "peaceful");
        assert_eq!(by_id("d2").memory_snippet, None);
    }

    #[test]
    fn test_achievement_roundtrip() {
        let db = test_db();
        assert!(db.save_achievement("ach1", "g1", "first_chat").unwrap());
        // 重复解锁返回 false（INSERT OR IGNORE）
        assert!(!db.save_achievement("ach2", "g1", "first_chat").unwrap());
        db.save_achievement("ach3", "g1", "love_50").unwrap();

        let rows = db.get_achievements("g1").unwrap();
        assert_eq!(rows.len(), 2);
        let keys: Vec<&str> = rows.iter().map(|r| r.achievement_key.as_str()).collect();
        assert!(keys.contains(&"first_chat"));
        assert!(keys.contains(&"love_50"));
    }

    #[test]
    fn test_diary_crud() {
        let db = test_db();
        db.save_diary("dy1", "g1", "今天主人夸我了", "2026-08-03").unwrap();
        db.save_diary("dy2", "g1", "今天主人没理我", "2026-08-04").unwrap();

        let entries = db.get_diary_entries("g1", 10).unwrap();
        assert_eq!(entries.len(), 2);
        let summaries: Vec<&str> = entries.iter().map(|r| r.summary.as_str()).collect();
        assert!(summaries.contains(&"今天主人夸我了"));
        assert!(summaries.contains(&"今天主人没理我"));
    }

    #[test]
    fn test_document_crud() {
        let db = test_db();
        db.save_document("doc1", "小说《流浪地球》", "C:/books/流浪地球.txt", "txt", 1000, "全文内容……")
            .unwrap();
        db.save_document("doc2", "论文", "C:/books/paper.docx", "docx", 500, "论文全文")
            .unwrap();

        let list = db.get_document_list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].import_status, "summarizing");
        assert!(list[0].summary_json.is_none());
        assert!(list[0].full_text.is_none());

        // 摘要完成 → ready
        db.update_document_summary("doc1", r#"{"chapters":[]}"#).unwrap();
        db.update_document_status("doc2", "failed", Some("解析失败")).unwrap();

        let ready = db.get_ready_documents().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "doc1");

        let detail = db.get_document_by_id("doc1").unwrap();
        assert_eq!(detail.full_text.as_deref(), Some("全文内容……"));
        assert_eq!(detail.summary_json.as_deref(), Some(r#"{"chapters":[]}"#));

        db.delete_document("doc1").unwrap();
        assert_eq!(db.get_document_list().unwrap().len(), 1);
        assert!(db.get_document_by_id("doc1").is_err());
    }

    #[test]
    fn test_conversation_chunk_crud() {
        let db = test_db();
        db.save_conversation_chunk("ch1", "g1", "讨论了项目架构", 0.7, Some(1200))
            .unwrap();
        db.save_conversation_chunk("ch2", "g1", "闲聊", 0.2, None).unwrap();

        let rows = db.get_conversation_chunks("g1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        let by_id = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(by_id("ch1").end_time.is_none());
        assert_eq!(by_id("ch2").importance, 0.2);

        db.close_conversation_chunk("ch2").unwrap();
        let rows = db.get_conversation_chunks("g1", 10).unwrap();
        assert!(rows.iter().find(|r| r.id == "ch2").unwrap().end_time.is_some());
        assert!(rows.iter().find(|r| r.id == "ch1").unwrap().end_time.is_none());
    }

    #[test]
    fn test_touch_memory_accessed() {
        let db = test_db();
        db.save_short_term_memory("stm1", "g1", "记忆", 0.5, 0.5, 0.0, None, None)
            .unwrap();
        db.save_long_term_memory("ltm1", "g1", "长期记忆", 0.5, false, None, None)
            .unwrap();
        db.touch_memory_accessed(&["stm1"], &["ltm1"]).unwrap();
        // 不崩溃即通过（LastAccessedAt 更新成功）
        assert!(db.count_short_term_memories("g1").unwrap() == 1);
    }
}