package storage

import (
	"context"
	"time"

	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) CreateExercise(ex models.Exercise) error {
	ctx := context.Background()
	_, err := s.db.ExecContext(ctx,
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

	err := s.db.QueryRow(
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
	return &ex, nil
}

func (s *Storage) GetExerciseByID(id string) (*models.Exercise, error) {
	var ex models.Exercise
	var createdAt string

	err := s.db.QueryRow(
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
	return &ex, nil
}
