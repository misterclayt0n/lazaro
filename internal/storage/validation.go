package storage

import (
	"database/sql"
	"fmt"
)

func (s *Storage) ExerciseExists(name string) (bool, error) {
	var exists bool
	err := s.db.QueryRow(
		"SELECT EXISTS(SELECT 1 FROM exercises WHERE name = ?)",
		name,
	).Scan(&exists)

	if err != nil && err != sql.ErrNoRows {
		return false, fmt.Errorf("failed to check exercise existence: %w", err)
	}

	return exists, nil
}
