package storage

import (
	"context"
	"time"

	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) CreateExercise(ex models.Exercise) error {
	ctx := context.Background()
	_, err := s.DB.ExecContext(ctx,
		`INSERT INTO exercises
		(id, name, description, primary_muscle, created_at, estimated_one_rm)
		VALUES (?, ?, ?, ?, ?, ?)`,
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
        ORDER BY (weight * (1 + reps/30)) DESC
        LIMIT 1`,
		ex.ID,
	).Scan(&bestSet.Weight, &bestSet.Reps)

	if err == nil {
		ex.BestSet = &bestSet
	}

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
        ORDER BY (weight * (1 + reps/30)) DESC
        LIMIT 1`,
		ex.ID,
	).Scan(&bestSet.Weight, &bestSet.Reps)

	if err == nil {
		ex.BestSet = &bestSet
	}

	return &ex, nil
}
