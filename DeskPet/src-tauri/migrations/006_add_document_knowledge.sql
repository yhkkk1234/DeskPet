CREATE TABLE IF NOT EXISTS DocumentKnowledge (
    Id TEXT PRIMARY KEY,
    Title TEXT NOT NULL,
    FilePath TEXT NOT NULL,
    FileType TEXT NOT NULL,
    WordCount INTEGER NOT NULL DEFAULT 0,
    SummaryJson TEXT NOT NULL DEFAULT '',
    FullText TEXT NOT NULL DEFAULT '',
    ImportStatus TEXT NOT NULL DEFAULT 'importing',
    ErrorMessage TEXT,
    CreatedAt TEXT NOT NULL DEFAULT (datetime('now')),
    UpdatedAt TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_doc_status ON DocumentKnowledge(ImportStatus);
