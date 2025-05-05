PRAGMA foreign_keys = ON;

-- Exercises -------------------------------------------------------------------
CREATE TABLE exercises (
    idx             INTEGER PRIMARY KEY AUTOINCREMENT,      -- numeric handle
    id              TEXT    UNIQUE,                         -- stable UUID
    name            TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    description     TEXT,
    primary_muscle  TEXT    NOT NULL CHECK (primary_muscle IN (
                     'biceps','triceps','forearms','chest','shoulders','back',
                     'quads','hamstrings','glutes','calves','abs')),
    created_at      TEXT    NOT NULL,
    current_pr_date TEXT,
    estimated_one_rm REAL
);

-- optional many‑to‑one aliases (handy for swap and fuzzy search)
CREATE TABLE exercise_aliases (
    exercise_id TEXT NOT NULL,          -- → exercises.id (uuid)
    alias       TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (exercise_id, alias),
    FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
);

CREATE TABLE exercise_variants (
    id          TEXT PRIMARY KEY,
    exercise_id INTEGER NOT NULL,       -- → exercises.idx  (numeric)
    name        TEXT NOT NULL COLLATE NOCASE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (exercise_id) REFERENCES exercises(idx) ON DELETE CASCADE,
    UNIQUE (exercise_id, name)
);

CREATE TABLE personal_records (
    exercise_id   TEXT NOT NULL,        -- → exercises.id
    date          TEXT NOT NULL,
    weight        REAL NOT NULL,
    reps          INTEGER NOT NULL,
    estimated_1rm REAL NOT NULL,
    PRIMARY KEY (exercise_id, date),
    FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
);

-- Programs --------------------------------------------------------------------
CREATE TABLE programs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description TEXT,
    created_at  TEXT NOT NULL
);

CREATE TABLE program_blocks (
    id          TEXT PRIMARY KEY,
    program_id  TEXT NOT NULL,          -- → programs.id
    name        TEXT NOT NULL COLLATE NOCASE,
    description TEXT,
    week        INTEGER,
    FOREIGN KEY (program_id) REFERENCES programs(id) ON DELETE CASCADE
);

CREATE TABLE program_exercises (
    id                 TEXT PRIMARY KEY,
    program_block_id   TEXT NOT NULL,   -- → program_blocks.id
    exercise_id        TEXT NOT NULL,   -- → exercises.id     (uuid)
    sets               INTEGER NOT NULL,
    reps               TEXT,            -- may be NULL for “simplest program”
    target_rpe         TEXT,
    target_rm_percent  TEXT,
    notes              TEXT,
    program_1rm        REAL,
    technique          TEXT,
    technique_group    INTEGER,
    order_index        INTEGER,
    FOREIGN KEY (program_block_id) REFERENCES program_blocks(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id)      REFERENCES exercises(id),
    UNIQUE(program_block_id, exercise_id)
);

-- Sessions --------------------------------------------------------------------
CREATE TABLE training_sessions (
    id               TEXT PRIMARY KEY,
    program_block_id TEXT NOT NULL,     -- → program_blocks.id
    start_time       TEXT NOT NULL,
    end_time         TEXT,
    notes            TEXT,
    FOREIGN KEY (program_block_id) REFERENCES program_blocks(id)
);

CREATE TABLE training_session_exercises (
    id                  TEXT PRIMARY KEY,
    training_session_id TEXT NOT NULL,  -- → training_sessions.id
    exercise_id         TEXT NOT NULL,  -- → exercises.id
    notes               TEXT,
    FOREIGN KEY (training_session_id) REFERENCES training_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id)         REFERENCES exercises(id)
);

CREATE TABLE exercise_sets (
    id                  TEXT PRIMARY KEY,
    session_exercise_id TEXT NOT NULL,  -- → training_session_exercises.id
    weight              REAL NOT NULL,
    reps                INTEGER NOT NULL,
    rpe                 REAL,
    rm_percent          REAL,
    notes               TEXT,
    timestamp           TEXT NOT NULL,
    ignore_for_one_rm   INTEGER DEFAULT 0,
    bodyweight          INTEGER DEFAULT 0,
    FOREIGN KEY (session_exercise_id) REFERENCES training_session_exercises(id)
                 ON DELETE CASCADE
);

-- Convenience view: the session that’s still open -----------------------------
CREATE VIEW current_session AS
SELECT *
FROM training_sessions
WHERE end_time IS NULL
LIMIT 1;

