-- Exercises and aliases -------------------------------------------------------
CREATE TABLE exercises (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description     TEXT,
    primary_muscle  TEXT,
    created_at      TEXT NOT NULL,
    current_pr_date TEXT,
    estimated_one_rm REAL
);

-- optional many-to-one aliases (handy for swap and fuzzy search)
CREATE TABLE exercise_aliases (
    exercise_id TEXT NOT NULL,
    alias       TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (exercise_id, alias),
    FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
);

CREATE TABLE personal_records (
    exercise_id   TEXT NOT NULL,
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
    program_id  TEXT NOT NULL,
    name        TEXT NOT NULL COLLATE NOCASE,
    description TEXT,
    week        INTEGER,
    FOREIGN KEY (program_id) REFERENCES programs(id) ON DELETE CASCADE
);

CREATE TABLE program_exercises (
    id                 TEXT PRIMARY KEY,
    program_block_id   TEXT NOT NULL,
    exercise_id        TEXT NOT NULL,
    sets               INTEGER NOT NULL,
    reps               TEXT NOT NULL,
    target_rpe         TEXT,
    target_rm_percent  TEXT,
    notes              TEXT,
    program_1rm        REAL,
    options            TEXT,
    technique          TEXT,
    technique_group    INTEGER,
    order_index        INTEGER,
    FOREIGN KEY (program_block_id) REFERENCES program_blocks(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id)      REFERENCES exercises(id),
    UNIQUE(program_block_id, exercise_id)
);

-- Training sessions -----------------------------------------------------------
CREATE TABLE training_sessions (
    id              TEXT PRIMARY KEY,
    program_block_id TEXT NOT NULL,
    start_time      TEXT NOT NULL,
    end_time        TEXT,
    notes           TEXT,
    FOREIGN KEY (program_block_id) REFERENCES program_blocks(id)
);

CREATE TABLE training_session_exercises (
    id                    TEXT PRIMARY KEY,
    training_session_id   TEXT NOT NULL,
    exercise_id           TEXT NOT NULL,
    notes                 TEXT,
    FOREIGN KEY (training_session_id) REFERENCES training_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_id)         REFERENCES exercises(id)
);

CREATE TABLE exercise_sets (
    id                  TEXT PRIMARY KEY,
    session_exercise_id TEXT NOT NULL,
    weight              REAL NOT NULL,
    reps                INTEGER NOT NULL,
    rpe                 REAL,
    rm_percent          REAL,
    notes               TEXT,
    timestamp           TEXT NOT NULL,
    ignore_for_one_rm   INTEGER DEFAULT 0,
    bodyweight          INTEGER DEFAULT 0,
    FOREIGN KEY (session_exercise_id) REFERENCES training_session_exercises(id) ON DELETE CASCADE
);

-- View that always yields “the session in progress” ---------------------------
CREATE VIEW current_session AS
SELECT *
FROM training_sessions
WHERE end_time IS NULL
LIMIT 1;

