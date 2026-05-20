CREATE TABLE IF NOT EXISTS Dreams (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    Summary TEXT NOT NULL,
    MemorySnippet TEXT,
    DreamType TEXT NOT NULL DEFAULT 'midnight',
    CreatedAt TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (GhostId) REFERENCES Ghosts(GhostId)
);

CREATE INDEX IF NOT EXISTS idx_dreams_ghost_created ON Dreams(GhostId, CreatedAt);
