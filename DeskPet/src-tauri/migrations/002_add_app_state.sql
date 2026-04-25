-- 应用状态表（持久化 TimelineState 等内存状态）
CREATE TABLE IF NOT EXISTS AppState (
    Id INTEGER PRIMARY KEY CHECK (Id = 1),
    LastInteraction TEXT,
    UpdatedAt TEXT NOT NULL
);

INSERT OR IGNORE INTO AppState (Id, UpdatedAt) VALUES (1, datetime('now'));
