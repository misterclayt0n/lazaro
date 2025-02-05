package storage

import (
	"database/sql"
	"time"

	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) GetPreviousSession(exerciseID, programBlockID string) (*models.TrainingSession, error) {
	row := s.DB.QueryRow(`
        SELECT ts.id, ts.start_time, ts.end_time
        FROM training_sessions ts
        JOIN training_session_exercises tse ON ts.id = tse.training_session_id
        WHERE tse.exercise_id = ?
        AND ts.program_block_id = ?
        AND ts.end_time IS NOT NULL  -- Only completed sessions
        ORDER BY ts.start_time DESC
        LIMIT 1
    `, exerciseID, programBlockID)

	var session models.TrainingSession
	var startTime string
	var endTime sql.NullString // Use NullString to handle NULL.

	err := row.Scan(&session.ID, &startTime, &endTime)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil // No previous session found.
		}
		return nil, err
	}

	// Parse start_time.
	session.StartTime, _ = time.Parse(time.RFC3339, startTime)

	// Handle NULL end_time.
	if endTime.Valid {
		session.EndTime = new(time.Time)
		*session.EndTime, _ = time.Parse(time.RFC3339, endTime.String)
	} else {
		session.EndTime = nil
	}

	return &session, nil
}

func (s *Storage) GetExerciseSetsForSession(sessionID, exerciseID string) ([]models.ExerciseSet, error) {
	var sessionExerciseID string
	err := s.DB.QueryRow(
		`SELECT id FROM training_session_exercises
		WHERE training_session_id = ? AND exercise_id = ?`,
		sessionID, exerciseID,
	).Scan(&sessionExerciseID)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	rows, err := s.DB.Query(`
		SELECT id, weight, reps, timestamp
		FROM exercise_sets
		WHERE session_exercise_id = ?
		ORDER BY timestamp ASC`,
		sessionExerciseID,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var sets []models.ExerciseSet
	for rows.Next() {
		var set models.ExerciseSet
		var rawTime string
		if err := rows.Scan(&set.ID, &set.Weight, &set.Reps, &rawTime); err != nil {
			continue
		}
		set.Timestamp, _ = time.Parse(time.RFC3339, rawTime)
		sets = append(sets, set)
	}
	return sets, nil
}

// GetSessionByID returns a TrainingSession (with its exercises and sets) by its session ID.
func (s *Storage) GetSessionByID(sessionID string) (*models.TrainingSession, error) {
	// Query the session basic data.
	var ts models.TrainingSession
	var startTime, endTimeStr string
	err := s.DB.QueryRow(`
        SELECT id, start_time, end_time, notes
        FROM training_sessions
        WHERE id = ?`, sessionID).Scan(&ts.ID, &startTime, &endTimeStr, &ts.Notes)
	if err != nil {
		return nil, err
	}

	ts.StartTime, _ = time.Parse(time.RFC3339, startTime)
	if endTimeStr != "" {
		t, _ := time.Parse(time.RFC3339, endTimeStr)
		ts.EndTime = &t
	}
	// Now get the exercises that belong to this session.
	rows, err := s.DB.Query(`
        SELECT id, exercise_id, notes
        FROM training_session_exercises
        WHERE training_session_id = ?`, sessionID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var exercises []models.SessionExercise
	for rows.Next() {
		var se models.SessionExercise
		if err := rows.Scan(&se.ID, &se.Exercise.ID, &se.SessionNotes); err != nil {
			continue
		}
		// Load the exercise details.
		exercise, err := s.GetExerciseByID(se.Exercise.ID)
		if err == nil {
			se.Exercise = *exercise
		}
		// Load the sets for this session exercise.
		se.Sets, _ = s.GetExerciseSetsForSession(sessionID, se.Exercise.ID)
		exercises = append(exercises, se)
	}
	ts.Exercises = exercises
	return &ts, nil
}

// GetSessionsByDate returns all training sessions whose start_time matches the given date (formatted as "2006-01-02").
func (s *Storage) GetSessionsByDate(dateStr string) ([]models.TrainingSession, error) {
    query := `
        SELECT id, start_time, end_time, notes
        FROM training_sessions
        WHERE date(start_time) = ?
        ORDER BY start_time DESC
    `
    rows, err := s.DB.Query(query, dateStr)
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
