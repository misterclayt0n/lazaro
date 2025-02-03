package storage

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) SaveSession(state *models.SessionState) error {
	ctx := context.Background()
	tx, err := s.DB.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("Failed to begin transaction: %w", err)
	}
	defer tx.Rollback()

	// Update the existing session with end_time.
	_, err = tx.ExecContext(ctx,
		`UPDATE training_sessions
         SET end_time = ?
         WHERE id = ?`,
		time.Now().UTC().Format(time.RFC3339),
		state.SessionID,
	)
	if err != nil {
		return fmt.Errorf("Failed to update session end time: %w", err)
	}

	// Save the training session.
	_, err = tx.ExecContext(ctx,
		`INSERT INTO training_sessions
		(id, program_block_id, start_time, end_time, notes)
		VALUES (?, ?, ?, ?, ?)`,
		state.SessionID,
		state.ProgramBlockID, // Assuming program_id is stored here.
		state.StartTime.Format(time.RFC3339),
		time.Now().UTC().Format(time.RFC3339), // end_time is current time.
		"",                                    // Add notes if needed.
	)
	if err != nil {
		return fmt.Errorf("Failed to create training session: %w", err)
	}

	// Save exercises and sets.
	for _, exercise := range state.Exercises {
		// Create session exercise.
		sessionExID := uuid.New().String()
		_, err = tx.ExecContext(ctx,
			`INSERT INTO training_session_exercises
            (id, training_session_id, exercise_id, notes)
            VALUES (?, ?, ?, ?)`,
			sessionExID,
			state.SessionID,
			exercise.Exercise.ID,
			exercise.SessionNotes,
		)
		if err != nil {
			return fmt.Errorf("Failed to create session exercise: %w", err)
		}

		// Save the sets.
		for _, set := range exercise.Sets {
			_, err = tx.ExecContext(ctx,
				`INSERT INTO exercise_sets
                (id, session_exercise_id, weight, reps, timestamp)
                VALUES (?, ?, ?, ?, ?)`,
				uuid.New().String(),
				sessionExID,
				set.Weight,
				set.Reps,
				set.Timestamp.Format(time.RFC3339),
			)
			if err != nil {
				return fmt.Errorf("Failed to save set: %w", err)
			}
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Failed to commit transaction: %w", err)
	}

	return nil
}

func (s *Storage) GetProgramByName(name string) (*models.Program, error) {
	var program models.Program
	var createdAt string

	err := s.DB.QueryRow(
		`SELECT id, name, description, created_at
        FROM programs WHERE name = ?`,
		name,
	).Scan(
		&program.ID,
		&program.Name,
		&program.Description,
		&createdAt,
	)

	if err != nil {
		return nil, err
	}

	program.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)

	// Load the program’s blocks.
	blockRows, err := s.DB.Query(`
        SELECT id, name, description
        FROM program_blocks
        WHERE program_id = ?
    `, program.ID)
	if err != nil {
		return nil, fmt.Errorf("Failed to load blocks: %w", err)
	}
	defer blockRows.Close()

	for blockRows.Next() {
		var block models.ProgramBlock
		if err := blockRows.Scan(
			&block.ID,
			&block.Name,
			&block.Description,
		); err != nil {
			return nil, fmt.Errorf("Failed to scan block: %w", err)
		}

		// Load exercises in each block.
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
			var repsJSON, targetRPEJSON, targetRMPercentJSON string // NOTE: Temporary variable to hold the JSON string.

			if err := exerciseRows.Scan(
				&ex.ID,
				&ex.ExerciseID,
				&ex.Sets,
				&repsJSON, // Scan into repsJSON here.
				&targetRPEJSON,
				&targetRMPercentJSON,
				&ex.ProgramNotes,
			    &ex.Program1RM,
			); err != nil {
				return nil, fmt.Errorf("Failed to scan exercise: %w", err)
			}

			// Unmarshal the JSON string into ex.Reps.
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
