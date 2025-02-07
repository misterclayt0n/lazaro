package storage

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/utils"
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

		// Process the sets:
		setsToSave := exercise.Sets
		if strings.EqualFold(exercise.Technique, models.TechniqueHell) {
			// Automatically filter/split the hell sets using your the threshold.
			// NOTE: I'm hard coding this thing for now because fuck you.
			setsToSave = processHellSets(exercise.Sets, 5)
		}

		// Save the sets.
		for _, set := range setsToSave {
			ignoreVal := 0
			if strings.EqualFold(exercise.Technique, models.TechniqueMyoreps) ||
				strings.EqualFold(exercise.Technique, models.TechniqueHell) {
				ignoreVal = 1
			}

			_, err = tx.ExecContext(ctx,
				`INSERT INTO exercise_sets
                (id, session_exercise_id, weight, reps, timestamp, ignore_for_one_rm, bodyweight)
                VALUES (?, ?, ?, ?, ?, ?, ?)`,
				uuid.New().String(),
				sessionExID,
				set.Weight,
				set.Reps,
				set.Timestamp.Format(time.RFC3339),
				ignoreVal,
			    utils.BoolToInt(set.Bodyweight),
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
        SELECT id, name, description, week
        FROM program_blocks
        WHERE program_id = ?
    `, program.ID)
	if err != nil {
		return nil, fmt.Errorf("Failed to load blocks: %w", err)
	}
	defer blockRows.Close()

	for blockRows.Next() {
		var block models.ProgramBlock
		var week sql.NullInt64
		if err := blockRows.Scan(
			&block.ID,
			&block.Name,
			&block.Description,
			&week,
		); err != nil {
			return nil, fmt.Errorf("Failed to scan block: %w", err)
		}

		if week.Valid {
			block.Week = int(week.Int64)
		} else {
			block.Week = 0 // Default fallback.
		}

		// Load exercises in each block.
		exerciseRows, err := s.DB.Query(`
		    SELECT id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm, COALESCE(options, '[]') AS options, technique, technique_group
		    FROM program_exercises
		    WHERE program_block_id = ?
			ORDER BY order_index
		`, block.ID)
		if err != nil {
			return nil, fmt.Errorf("Failed to load exercises: %w", err)
		}
		defer exerciseRows.Close()

		for exerciseRows.Next() {
			var ex models.ProgramExercise
			var repsJSON, targetRPEJSON, targetRMPercentJSON, optionsJSON string // NOTE: Temporary variable to hold the JSON string.

			if err := exerciseRows.Scan(
				&ex.ID,
				&ex.ExerciseID,
				&ex.Sets,
				&repsJSON, // Scan into repsJSON here.
				&targetRPEJSON,
				&targetRMPercentJSON,
				&ex.ProgramNotes,
				&ex.Program1RM,
				&optionsJSON,
				&ex.Technique,
				&ex.TechniqueGroup,
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

			if err := json.Unmarshal([]byte(optionsJSON), &ex.Options); err != nil {
				return nil, fmt.Errorf("Failed to unmarshal options: %w", err)
			}

			block.Exercises = append(block.Exercises, ex)
		}

		program.Blocks = append(program.Blocks, block)
	}

	return &program, nil
}

// GetTrainingSessionsForExercise returns up to "limit" training sessions in which the given exercise was performed.
func (s *Storage) GetTrainingSessionsForExercise(exerciseID string, limit int) ([]models.TrainingSession, error) {
	query := `
		SELECT DISTINCT ts.id, ts.start_time, ts.end_time, ts.notes
		FROM training_sessions ts
		JOIN training_session_exercises tse ON ts.id = tse.training_session_id
		WHERE tse.exercise_id = ?
		ORDER BY ts.start_time DESC
		LIMIT ?
    `

	rows, err := s.DB.Query(query, exerciseID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var sessions []models.TrainingSession
	for rows.Next() {
		var ts models.TrainingSession
		var startTime, endTimeStr string
		if err := rows.Scan(&ts.ID, &startTime, &endTimeStr, &ts.Notes); err != nil {
			continue
		}
		ts.StartTime, _ = time.Parse(time.RFC3339, startTime)
		if endTimeStr != "" {
			t, _ := time.Parse(time.RFC3339, endTimeStr)
			ts.EndTime = &t
		}
		sessions = append(sessions, ts)
	}

	return sessions, nil
}

func processHellSets(sets []models.ExerciseSet, minReps int) []models.ExerciseSet {
	var processed []models.ExerciseSet
	for _, s := range sets {
		// Only keep sets that have at least the minimum reps.
		if s.Reps < minReps {
			break // End the hell chain when reps drop below the minimum.
		}
		// Mark these sets to be ignored for 1RM.
		s.IgnoreForOneRM = true
		processed = append(processed, s)
	}
	return processed
}
