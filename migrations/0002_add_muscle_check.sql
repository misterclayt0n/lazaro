-- new migration
ALTER TABLE exercises RENAME TO _exercises_old;

CREATE TABLE exercises (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description TEXT,
    primary_muscle TEXT NOT NULL
        CHECK (primary_muscle IN ('biceps','triceps', 'forearms', 'chest', 'shoulders', 'back', 'quads', 'hamstrings', 'glutes', 'calves', 'abs')),
    created_at TEXT NOT NULL
);

INSERT INTO exercises (id, name, description, primary_muscle, created_at)
SELECT id, name, description, primary_muscle, created_at
FROM _exercises_old;

DROP TABLE _exercises_old;

