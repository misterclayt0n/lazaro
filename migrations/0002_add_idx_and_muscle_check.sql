PRAGMA foreign_keys = off;

ALTER TABLE exercises RENAME TO _exercises_old;

CREATE TABLE exercises (
    idx             INTEGER PRIMARY KEY AUTOINCREMENT,
    id              TEXT UNIQUE,
    name            TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description     TEXT,
    primary_muscle  TEXT NOT NULL
        CHECK (primary_muscle IN (
            'biceps','triceps','forearms','chest','shoulders','back',
            'quads','hamstrings','glutes','calves','abs'
        )),
    created_at      TEXT NOT NULL,
    current_pr_date TEXT,
    estimated_one_rm REAL
);

INSERT INTO exercises
      (idx, id, name, description, primary_muscle,
       created_at, current_pr_date, estimated_one_rm)
SELECT ROW_NUMBER() OVER (ORDER BY name)     AS idx,
       id,
       name,
       description,
       primary_muscle,
       created_at,
       current_pr_date,
       estimated_one_rm
FROM _exercises_old;

DROP TABLE _exercises_old;

CREATE UNIQUE INDEX exercises_name_idx ON exercises(name);

PRAGMA foreign_keys = on;

