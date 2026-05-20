CREATE TABLE IF NOT EXISTS Achievements (
    Id TEXT PRIMARY KEY,
    GhostId TEXT NOT NULL,
    AchievementKey TEXT NOT NULL,
    UnlockedAt TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (GhostId) REFERENCES Ghosts(GhostId),
    UNIQUE(GhostId, AchievementKey)
);

CREATE INDEX IF NOT EXISTS idx_achievements_ghost ON Achievements(GhostId);
