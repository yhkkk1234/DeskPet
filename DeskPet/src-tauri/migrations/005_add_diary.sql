CREATE TABLE IF NOT EXISTS Diary (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    Summary TEXT NOT NULL,
    EntryDate TEXT NOT NULL,
    CreatedAt TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (GhostId) REFERENCES Ghosts(GhostId),
    UNIQUE(GhostId, EntryDate)
);

CREATE INDEX IF NOT EXISTS idx_diary_ghost_date ON Diary(GhostId, EntryDate);
