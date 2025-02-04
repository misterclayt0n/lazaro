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

			// Marshal the new slice fields.
			targetRPEJSON, err := json.Marshal(exerciseTOML.TargetRPE)
			if err != nil {
				return fmt.Errorf("Failed to marshal target_rpe: %w", err)
			}
			targetRMPercentJSON, err := json.Marshal(exerciseTOML.TargetRMPercent)
			if err != nil {
				return fmt.Errorf("Failed to marshal target_rm_percent: %w", err)
			}

			// Create program exercise.
			_, err = tx.ExecContext(ctx,
				`INSERT INTO program_exercises
		     (id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm)
		     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				uuid.New().String(),
				blockID,
				exerciseID,
				exerciseTOML.Sets,
				string(repsJSON),
				string(targetRPEJSON),
				string(targetRMPercentJSON),
				exerciseTOML.ProgramNotes,
				exerciseTOML.Program1RM,
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
			var repsJSON, targetRPEJSON, targetRMPercentJSON string // NOTE: Temporary storage for JSON string.

			err := exerciseRows.Scan(
				&ex.ID,
				&ex.ExerciseID,
				&ex.Sets,
				&repsJSON,
				&targetRPEJSON,
				&targetRMPercentJSON,
				&ex.ProgramNotes,
				&ex.Program1RM,
			)
			if err != nil {
				return nil, fmt.Errorf("Failed to scan exercise: %w", err)
			}

			if err := json.Unmarshal([]byte(repsJSON), &ex.Reps); err != nil {
				return nil, fmt.Errorf("Failed to unmarshal reps: %w", err)
			}

			if err := json.Unmarshal([]byte(targetRPEJSON), &ex.TargetRPE); err != nil {
				return nil, fmt.Errorf("Failed to unmarshal target_rpe: %w", err)
			}

			if err := json.Unmarshal([]byte(targetRMPercentJSON), &ex.TargetRMPercent); err != nil {
				return nil, fmt.Errorf("Failed to unmarshal target_rm_percent: %w", err)
			}

			block.Exercises = append(block.Exercises, ex)
		}

		program.Blocks = append(program.Blocks, block)
	}

	return &program, nil
}

// UpdateProgram updates the existing program based on a TOML file.
// It updates only the program and block/exercise fields so that existing sessions are not lost.
func (s *Storage) UpdateProgram(tomlData []byte) error {
	// Parse the TOML file into a ProgramTOML structure.
	var progTOML models.ProgramTOML
	if err := toml.Unmarshal(tomlData, &progTOML); err != nil {
		return fmt.Errorf("Invalid TOML format: %w", err)
	}

	// Retrieve the existing program by name.
	// NOTE: This assumes program name is unique.
	existingProgram, err := s.GetProgramByName(progTOML.Name)
	if err != nil {
		return fmt.Errorf("Failed to get existing program: %w", err)
	}

	ctx := context.Background()
	tx, err := s.DB.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("Failed to begin transaction: %w", err)
	}
	// In case of error, ensure the transaction is rolled back.
	defer tx.Rollback()

	// Update the program’s description if it has changed.
	if existingProgram.Description != progTOML.Description {
		_, err := tx.ExecContext(ctx, `UPDATE programs SET description = ? WHERE id = ?`,
			progTOML.Description, existingProgram.ID)
		if err != nil {
			return fmt.Errorf("Failed to update program: %w", err)
		}
	}

	// Iterate over each block in the TOML file.
	// For each block, try to match an existing block by name.
	// NOTE: Here, we assume block names are unique per program.
	for _, newBlock := range progTOML.Blocks {
		// Try to find an existing block with the same name.
		var blockID string
		err := tx.QueryRowContext(ctx, `SELECT id FROM program_blocks WHERE program_id = ? AND name = ?`,
			existingProgram.ID, newBlock.Name).Scan(&blockID)
		if err != nil {
			// If no block exists, then insert a new block.
			if err == sql.ErrNoRows {
				blockID = generateID() // generateID() is a helper that returns a new UUID.
				_, err = tx.ExecContext(ctx, `INSERT INTO program_blocks (id, program_id, name, description)
					VALUES (?, ?, ?, ?)`, blockID, existingProgram.ID, newBlock.Name, newBlock.Description)
				if err != nil {
					return fmt.Errorf("Failed to insert new block: %w", err)
				}
			} else {
				return fmt.Errorf("Failed to query program block: %w", err)
			}
		} else {
			// If the block exists, update its description if necessary.
			_, err = tx.ExecContext(ctx, `UPDATE program_blocks SET description = ? WHERE id = ?`,
				newBlock.Description, blockID)
			if err != nil {
				return fmt.Errorf("Failed to update block: %w", err)
			}
		}

		// Process exercises in the block.
		for _, newEx := range newBlock.Exercises {
			// Get the exercise id from the exercises table by name.
			var exerciseID string
			err := tx.QueryRowContext(ctx, `SELECT id FROM exercises WHERE name = ?`, newEx.Name).Scan(&exerciseID)
			if err != nil {
				if err == sql.ErrNoRows {
					// You might want to return an error or choose to create the exercise.
					return fmt.Errorf("Exercise '%s' not found", newEx.Name)
				}
				return fmt.Errorf("Failed to query exercise: %w", err)
			}

			// Marshal the reps into JSON.
			repsJSON, err := json.Marshal(newEx.Reps)
			if err != nil {
				return fmt.Errorf("Failed to marshal reps: %w", err)
			}

			// Marshal target RPE and target RM Percent into JSON.
			targetRPEJSON, err := json.Marshal(newEx.TargetRPE)
			if err != nil {
				return fmt.Errorf("Failed to marshal target_rpe: %w", err)
			}
			targetRMPercentJSON, err := json.Marshal(newEx.TargetRMPercent)
			if err != nil {
				return fmt.Errorf("Failed to marshal target_rm_percent: %w", err)
			}

			// Check if a program_exercise for this exercise in this block already exists.
			var peID string
			err = tx.QueryRowContext(ctx, `SELECT id FROM program_exercises
				WHERE program_block_id = ? AND exercise_id = ?`, blockID, exerciseID).Scan(&peID)
			if err != nil {
				if err == sql.ErrNoRows {
					// Insert a new program exercise.
					peID = generateID()
					_, err = tx.ExecContext(ctx, `
						INSERT INTO program_exercises
						(id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm)
						VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
						peID, blockID, exerciseID, newEx.Sets, string(repsJSON),
						newEx.TargetRPE, newEx.TargetRMPercent, newEx.ProgramNotes, newEx.Program1RM,
					)
					if err != nil {
						return fmt.Errorf("Failed to insert program exercise: %w", err)
					}
				} else {
					return fmt.Errorf("Failed to query program exercise: %w", err)
				}
			} else {
				// Update the program exercise fields.
				_, err = tx.ExecContext(ctx, `
					UPDATE program_exercises SET sets = ?, reps = ?, target_rpe = ?, target_rm_percent = ?, notes = ?, program_1rm = ?
					WHERE id = ?`,
					newEx.Sets, string(repsJSON), string(targetRPEJSON), string(targetRMPercentJSON), newEx.ProgramNotes, newEx.Program1RM,
					peID,
				)
				if err != nil {
					return fmt.Errorf("Failed to update program exercise: %w", err)
				}
			}
		}
	}

	// Commit the transaction.
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Failed to commit transaction: %w", err)
	}

	return nil
}

func (s *Storage) DeleteProgramByName(name string) error {
	ctx := context.Background()

	// First, find the program ID by name.
	var programID string
	err := s.DB.QueryRowContext(ctx, `SELECT id FROM programs WHERE name = ?`, name).Scan(&programID)
	if err != nil {
		return fmt.Errorf("Program not found: %w", err)
	}

	// Delete the program row.
	_, err = s.DB.ExecContext(ctx, `DELETE FROM programs WHERE id = ?`, programID)
	if err != nil {
		return fmt.Errorf("Failed to delete program: %w", err)
	}

	return nil
}

func generateID() string {
	return uuid.New().String()
}
