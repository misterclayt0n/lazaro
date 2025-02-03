package storage

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) CreateProgram(tomlData []byte) error {
	ctx := context.Background()
	tx, err := s.DB.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("Failed to begin transaction: %w", err)
	}
	defer tx.Rollback()

	// Parse TOML.
	var programTOML models.ProgramTOML
	if err := toml.Unmarshal(tomlData, &programTOML); err != nil {
		return fmt.Errorf("Invalid TOML format: %w", err)
	}

	// Create main program.
	programID := uuid.New().String()
	createdAt := time.Now().UTC().Format(time.RFC3339)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO programs (id, name, description, created_at)
         VALUES (?, ?, ?, ?)`,
		programID,
		programTOML.Name,
		programTOML.Description,
		createdAt,
	)
	if err != nil {
		return fmt.Errorf("Failed to create program: %w", err)
	}

	// Process blocks.
	for _, blockTOML := range programTOML.Blocks {
		blockID := uuid.New().String()
		_, err = tx.ExecContext(ctx,
			`INSERT INTO program_blocks
             (id, program_id, name, description)
             VALUES (?, ?, ?, ?)`,
			blockID,
			programID,
			blockTOML.Name,
			blockTOML.Description,
		)
		if err != nil {
			return fmt.Errorf("Failed to create program block: %w", err)
		}

		// Process exercises in block.
		for _, exerciseTOML := range blockTOML.Exercises {
			// Get exercise ID from name.
			var exerciseID string
			err := tx.QueryRowContext(ctx,
				"SELECT id FROM exercises WHERE name = ?",
				exerciseTOML.Name,
			).Scan(&exerciseID)
			if err != nil {
				if err == sql.ErrNoRows {
					return fmt.Errorf("exercise '%s' not found", exerciseTOML.Name)
				}
				return fmt.Errorf("Failed to validate exercise: %w", err)
			}

			repsJSON, err := json.Marshal(exerciseTOML.Reps)
			if err != nil {
				return fmt.Errorf("Failed to marshal reps: %w", err)
			}

			// Create program exercise.
			_, err = tx.ExecContext(ctx,
				`INSERT INTO program_exercises
		     (id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm)
		     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`, // Added program_1rm
				uuid.New().String(),
				blockID,
				exerciseID,
				exerciseTOML.Sets,
				string(repsJSON),
				exerciseTOML.TargetRPE,
				exerciseTOML.TargetRMPercent,
				exerciseTOML.ProgramNotes,
				exerciseTOML.Program1RM, // New field
			)
			if err != nil {
				return fmt.Errorf("Failed to create program exercise: %w", err)
			}
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Failed to commit transaction: %w", err)
	}

	return nil
}

func (s *Storage) ListPrograms() ([]models.Program, error) {
	rows, err := s.DB.Query(`
        SELECT id, name, description, created_at
        FROM programs
    `)
	if err != nil {
		return nil, fmt.Errorf("Failed to query programs: %w", err)
	}
	defer rows.Close()

	var programs []models.Program
	for rows.Next() {
		var p models.Program
		var createdAt string

		err := rows.Scan(
			&p.ID,
			&p.Name,
			&p.Description,
			&createdAt,
		)
		if err != nil {
			return nil, fmt.Errorf("Failed to scan program: %w", err)
		}

		p.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
		programs = append(programs, p)
	}

	return programs, nil
}

func (s *Storage) GetProgram(id string) (*models.Program, error) {
	// Load program base.
	var program models.Program
	var createdAt string

	err := s.DB.QueryRow(`
        SELECT id, name, description, created_at
        FROM programs WHERE id = ?
    `, id).Scan(
		&program.ID,
		&program.Name,
		&program.Description,
		&createdAt,
	)
	if err != nil {
		return nil, fmt.Errorf("Program not found: %w", err)
	}
	program.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)

	// Load blocks.
	blockRows, err := s.DB.Query(`
        SELECT id, name, description
        FROM program_blocks
        WHERE program_id = ?
    `, id)
	if err != nil {
		return nil, fmt.Errorf("Failed to load blocks: %w", err)
	}
	defer blockRows.Close()

	for blockRows.Next() {
		var block models.ProgramBlock
		err := blockRows.Scan(
			&block.ID,
			&block.Name,
			&block.Description,
		)
		if err != nil {
			return nil, fmt.Errorf("Failed to scan block: %w", err)
		}

		// Load exercises.
		exerciseRows, err := s.DB.Query(`
		    SELECT id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm
		    FROM program_exercises
		    WHERE program_block_id = ?
		`, block.ID)
		if err != nil {
			return nil, fmt.Errorf("Failed to load exercises: %w", err)
		}
		defer exerciseRows.Close()

		for exerciseRows.Next() {
			var ex models.ProgramExercise
			var repsJSON string // NOTE: Temporary storage for JSON string.

			err := exerciseRows.Scan(
				&ex.ID,
				&ex.ExerciseID,
				&ex.Sets,
				&repsJSON,
				&ex.TargetRPE,
				&ex.TargetRMPercent,
				&ex.ProgramNotes,
			    &ex.Program1RM,
			)
			if err != nil {
				return nil, fmt.Errorf("Failed to scan exercise: %w", err)
			}

			if err := json.Unmarshal([]byte(repsJSON), &ex.Reps); err != nil {
				return nil, fmt.Errorf("Failed to unmarshal reps: %w", err)
			}

			block.Exercises = append(block.Exercises, ex)
		}

		program.Blocks = append(program.Blocks, block)
	}

	return &program, nil
}
