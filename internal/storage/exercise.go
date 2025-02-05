package storage

import (
	"context"
	"time"

	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/utils"
)

func (s *Storage) CreateExercise(ex models.Exercise) error {
	ctx := context.Background()
	ex.EstimatedOneRM = utils.CalculateInitialOneRM()

	_, err := s.DB.ExecContext(ctx,
		`INSERT INTO exercises
			(id, name, description, primary_muscle, created_at, estimated_one_rm)
			VALUES (?, ?, ?, ?, ?, ?)
			ON CONFLICT(name) DO UPDATE SET
				description = excluded.description,
				primary_muscle = excluded.primary_muscle,
				created_at = excluded.created_at,
				estimated_one_rm = excluded.estimated_one_rm`,
		ex.ID,
		ex.Name,
		ex.Description,
		ex.PrimaryMuscle,
		ex.CreatedAt.Format(time.RFC3339),
		ex.EstimatedOneRM,
	)
	return err
}

func (s *Storage) GetExerciseByName(name string) (*models.Exercise, error) {
	var ex models.Exercise
	var createdAt string

	err := s.DB.QueryRow(
		`SELECT id, name, description, primary_muscle, created_at, estimated_one_rm
		FROM exercises WHERE name = ?`,
		name,
	).Scan(
		&ex.ID,
		&ex.Name,
		&ex.Description,
		&ex.PrimaryMuscle,
		&createdAt,
		&ex.EstimatedOneRM,
	)

	if err != nil {
		return nil, err
	}

	ex.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)

	// Fetch the last performed date.
	var lastPerformedStr string
	err = s.DB.QueryRow(`
        SELECT MAX(ts.start_time)
        FROM training_sessions ts
        JOIN training_session_exercises tse ON ts.id = tse.training_session_id
        WHERE tse.exercise_id = ?
    `, ex.ID).Scan(&lastPerformedStr)
	if err == nil && lastPerformedStr != "" {
		ex.LastPerformed, _ = time.Parse(time.RFC3339, lastPerformedStr)
	}

	var bestSet models.ExerciseSet
	err = s.DB.QueryRow(`
        SELECT weight, reps
        FROM exercise_sets
        WHERE session_exercise_id IN (
            SELECT id FROM training_session_exercises WHERE exercise_id = ?
        )
        AND ignore_for_one_rm = 0
        ORDER BY (weight * (1 + reps/30)) DESC
        LIMIT 1`,
		ex.ID,
	).Scan(&bestSet.Weight, &bestSet.Reps)

	if err == nil {
		ex.BestSet = &bestSet
	}

	// Recalculate the estimate 1-RM.
	ex.EstimatedOneRM = utils.CalculateEpley1RM(bestSet.Weight, bestSet.Reps)

	return &ex, nil
}

func (s *Storage) GetExerciseByID(id string) (*models.Exercise, error) {
	var ex models.Exercise
	var createdAt string

	err := s.DB.QueryRow(
		`SELECT id, name, description, primary_muscle, created_at, estimated_one_rm
        FROM exercises WHERE id = ?`,
		id,
	).Scan(
		&ex.ID,
		&ex.Name,
		&ex.Description,
		&ex.PrimaryMuscle,
		&createdAt,
		&ex.EstimatedOneRM,
	)

	if err != nil {
		return nil, err
	}
	ex.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)

	// Fetch the last performed date.
	var lastPerformedStr string
	err = s.DB.QueryRow(`
        SELECT MAX(ts.start_time)
        FROM training_sessions ts
        JOIN training_session_exercises tse ON ts.id = tse.training_session_id
        WHERE tse.exercise_id = ?
    `, ex.ID).Scan(&lastPerformedStr)
	if err == nil && lastPerformedStr != "" {
		ex.LastPerformed, _ = time.Parse(time.RFC3339, lastPerformedStr)
	}

	var bestSet models.ExerciseSet
	err = s.DB.QueryRow(`
        SELECT weight, reps
        FROM exercise_sets
        WHERE session_exercise_id IN (
            SELECT id FROM training_session_exercises WHERE exercise_id = ?
        )
        AND ignore_for_one_rm = 0
        ORDER BY (weight * (1 + reps/30)) DESC
        LIMIT 1`,
		ex.ID,
	).Scan(&bestSet.Weight, &bestSet.Reps)

	if err == nil {
		ex.BestSet = &bestSet
	}

	// Recalculate the estimate 1-RM.
	ex.EstimatedOneRM = utils.CalculateEpley1RM(bestSet.Weight, bestSet.Reps)

	return &ex, nil
}
