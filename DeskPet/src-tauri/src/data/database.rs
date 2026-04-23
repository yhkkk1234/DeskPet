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
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&mut self) -> Result<(), String> {
        let migration_sql = include_str!("../../migrations/001_initial.sql");
        self.conn
            .execute_batch(migration_sql)
            .map_err(|e| format!("Migration failed: {}", e))?;
        Ok(())
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
                "SELECT Id, GhostId, Name, Summary, Source, Proficiency, LastUsedAt FROM Experiences WHERE GhostId = ?1 ORDER BY Proficiency DESC",
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
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(rows)
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
        let existing: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM MasterImpressions WHERE GhostId = ?1",
                params![ghost_id],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if existing {
            self.conn
                .execute(
                    "UPDATE MasterImpressions SET OpennessScore=?2, ConscientiousnessScore=?3, ExtraversionScore=?4, AgreeablenessScore=?5, NeuroticismScore=?6, CreativityScore=?7, OverallAffinity=?8, Snippets=?9, UpdatedAt=datetime('now') WHERE GhostId=?1",
                    params![ghost_id, openness, conscientiousness, extraversion, agreeableness, neuroticism, creativity, affinity, snippets],
                )
                .map_err(|e| e.to_string())?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            self.conn
                .execute(
                    "INSERT INTO MasterImpressions (Id, GhostId, OpennessScore, ConscientiousnessScore, ExtraversionScore, AgreeablenessScore, NeuroticismScore, CreativityScore, OverallAffinity, Snippets, UpdatedAt)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
                    params![id, ghost_id, openness, conscientiousness, extraversion, agreeableness, neuroticism, creativity, affinity, snippets],
                )
                .map_err(|e| e.to_string())?;
        }
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
