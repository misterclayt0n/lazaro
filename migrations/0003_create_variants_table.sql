PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS exercise_variants (
    id          TEXT PRIMARY KEY,                    
    exercise_id INTEGER NOT NULL,                    
    name        TEXT NOT NULL COLLATE NOCASE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (exercise_id) REFERENCES exercises(idx) ON DELETE CASCADE,
    UNIQUE (exercise_id, name)                       
);

PRAGMA foreign_keys = ON;
