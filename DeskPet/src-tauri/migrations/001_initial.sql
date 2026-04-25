-- 短期记忆表
CREATE TABLE IF NOT EXISTS ShortTermMemories (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    RawContent TEXT,
    Summary TEXT NOT NULL,
    Entities TEXT,
    Sentiment REAL DEFAULT 0,
    Importance REAL DEFAULT 0.3,
    Accessibility REAL DEFAULT 0.5,
    IsCompressed INTEGER DEFAULT 0,
    Category TEXT,
    CreatedAt TEXT NOT NULL,
    LastAccessedAt TEXT
);

-- 长期记忆表
CREATE TABLE IF NOT EXISTS LongTermMemories (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    Summary TEXT NOT NULL,
    Detail TEXT,
    Importance REAL NOT NULL,
    IsCoreMemory INTEGER DEFAULT 0,
    IsBlurred INTEGER DEFAULT 0,
    EventType TEXT,
    CreatedAt TEXT NOT NULL,
    LastAccessedAt TEXT
);

-- 经验表
CREATE TABLE IF NOT EXISTS Experiences (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    Name TEXT NOT NULL,
    Summary TEXT NOT NULL,
    Source TEXT NOT NULL,
    Proficiency REAL DEFAULT 0.3,
    LastUsedAt TEXT,
    SourceMemoryId TEXT
);

-- 会话片段表
CREATE TABLE IF NOT EXISTS ConversationChunks (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    StartTime TEXT NOT NULL,
    EndTime TEXT,
    Summary TEXT NOT NULL,
    Importance REAL DEFAULT 0.5,
    TokenCount INTEGER
);

-- 主人印象表
CREATE TABLE IF NOT EXISTS MasterImpressions (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    OpennessScore REAL DEFAULT 0,
    ConscientiousnessScore REAL DEFAULT 0,
    ExtraversionScore REAL DEFAULT 0,
    AgreeablenessScore REAL DEFAULT 0,
    NeuroticismScore REAL DEFAULT 0,
    CreativityScore REAL DEFAULT 0,
    OverallAffinity REAL DEFAULT 0,
    Snippets TEXT,
    UpdatedAt TEXT NOT NULL
);

-- 情感事件表
CREATE TABLE IF NOT EXISTS EmotionalEvents (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    EventType TEXT NOT NULL,
    Intensity REAL NOT NULL,
    LoveHateDelta REAL NOT NULL,
    BaselineDelta REAL NOT NULL,
    Description TEXT,
    CreatedAt TEXT NOT NULL
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_stm_ghost ON ShortTermMemories(GhostId);
CREATE INDEX IF NOT EXISTS idx_stm_importance ON ShortTermMemories(GhostId, Importance DESC);
CREATE INDEX IF NOT EXISTS idx_ltm_ghost ON LongTermMemories(GhostId);
CREATE INDEX IF NOT EXISTS idx_ltm_importance ON LongTermMemories(GhostId, Importance DESC);
CREATE INDEX IF NOT EXISTS idx_ltm_core ON LongTermMemories(GhostId, IsCoreMemory);
CREATE INDEX IF NOT EXISTS idx_exp_ghost ON Experiences(GhostId);
CREATE INDEX IF NOT EXISTS idx_cc_ghost ON ConversationChunks(GhostId);
CREATE INDEX IF NOT EXISTS idx_mi_ghost ON MasterImpressions(GhostId);
CREATE INDEX IF NOT EXISTS idx_ee_ghost ON EmotionalEvents(GhostId);

-- 聊天消息表
CREATE TABLE IF NOT EXISTS ChatMessages (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    Role TEXT NOT NULL,
    Content TEXT NOT NULL,
    CreatedAt TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cm_ghost ON ChatMessages(GhostId);
CREATE INDEX IF NOT EXISTS idx_cm_created ON ChatMessages(GhostId, CreatedAt DESC);