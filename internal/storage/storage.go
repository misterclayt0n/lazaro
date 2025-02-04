package storage

import (
	"database/sql"
	"fmt"
	"os"
	"time"

	"github.com/google/uuid"
	"github.com/joho/godotenv"
	_ "github.com/tursodatabase/libsql-client-go/libsql"
)

type Storage struct {
	DB *sql.DB
}

func NewStorage() *Storage {
	if err := godotenv.Load(); err != nil {
		fmt.Fprintf(os.Stderr, "No .env file found")
		os.Exit(1)
	}

	url := os.Getenv("TURSO_DATABASE_URL")
	if url == "" {
		fmt.Fprintf(os.Stderr, "TURSO_DATABASE_URL not set in the enviroment")
		os.Exit(1)
	}

	db, err := sql.Open("libsql", url)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to open db %s: %s", url, err)
		os.Exit(1)
	}

	if err := initializeDB(db); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize database: %v", err)
		os.Exit(1)
	}

	return &Storage{DB: db}
}

func initializeDB(db *sql.DB) error {
	_, err := db.Exec(`
        CREATE TABLE IF NOT EXISTS exercises (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            primary_muscle TEXT,
            created_at TEXT NOT NULL,
            current_pr_date TEXT,
            estimated_one_rm REAL
        );

        CREATE TABLE IF NOT EXISTS personal_records (
            exercise_id TEXT NOT NULL,
            date TEXT NOT NULL,
            weight REAL NOT NULL,
            reps INTEGER NOT NULL,
            estimated_1rm REAL NOT NULL,
            PRIMARY KEY (exercise_id, date),
            FOREIGN KEY (exercise_id) REFERENCES exercises(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS programs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS program_blocks (
            id TEXT PRIMARY KEY,
            program_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            week INTEGER,
            FOREIGN KEY (program_id) REFERENCES programs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS program_exercises (
            id TEXT PRIMARY KEY,
            program_block_id TEXT NOT NULL,
            exercise_id TEXT NOT NULL,
            sets INTEGER NOT NULL,
            reps TEXT NOT NULL,
            target_rpe TEXT,
            target_rm_percent TEXT,
            notes TEXT,
            program_1rm REAL,
            FOREIGN KEY (program_block_id) REFERENCES program_blocks(id) ON DELETE CASCADE,
            FOREIGN KEY (exercise_id) REFERENCES exercises(id)
        );

        CREATE TABLE IF NOT EXISTS training_sessions (
            id TEXT PRIMARY KEY,
            program_block_id TEXT NOT NULL,  -- Changed from program_session_id
            start_time TEXT NOT NULL,
            end_time TEXT,
            notes TEXT,
            FOREIGN KEY (program_block_id) REFERENCES program_blocks(id)
        );

        CREATE TABLE IF NOT EXISTS training_session_exercises (
            id TEXT PRIMARY KEY,
            training_session_id TEXT NOT NULL,
            exercise_id TEXT NOT NULL,
            notes TEXT,
            FOREIGN KEY (training_session_id) REFERENCES training_sessions(id) ON DELETE CASCADE,
            FOREIGN KEY (exercise_id) REFERENCES exercises(id)
        );

        CREATE TABLE IF NOT EXISTS exercise_sets (
            id TEXT PRIMARY KEY,
            session_exercise_id TEXT NOT NULL,
            weight REAL NOT NULL,
            reps INTEGER NOT NULL,
            rpe REAL,
            rm_percent REAL,
            notes TEXT,
            timestamp TEXT NOT NULL,
            FOREIGN KEY (session_exercise_id) REFERENCES training_session_exercises(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS current_session (
            session_id TEXT PRIMARY KEY,
            FOREIGN KEY (session_id) REFERENCES training_sessions(id) ON DELETE CASCADE
        );
    `)
	return err
}

func (s *Storage) StartSession(programName string) (string, error) {
	sessionID := uuid.New().String()
	startTime := time.Now().UTC().Format(time.RFC3339)

	_, err := s.DB.Exec(
		"INSERT INTO training_sessions (id, program, start_time) VALUES (?, ?, ?)",
		sessionID, programName, startTime,
	)
	if err != nil {
		return "", fmt.Errorf("failed to create session: %w", err)
	}

	_, err = s.DB.Exec(
		"INSERT OR REPLACE INTO current_session (id) VALUES (?)",
		sessionID,
	)
	if err != nil {
		return "", fmt.Errorf("Failed to set current session: %w", err)
	}

	return sessionID, nil
}
