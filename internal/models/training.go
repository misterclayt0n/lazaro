package models

import "time"

type TrainingSession struct {
	ID            string            `json:"id"`
	Program       Program           `json:"program"`
	StartTime     time.Time         `json:"start_time"`
	EndTime       *time.Time        `json:"end_time,omitempty"`
	Exercises     []SessionExercise `json:"exercises"`
	Notes         string            `json:"notes"`
}

type PersonalRecord struct {
	ExerciseName string    `json:"exercise_name"`
	Weight       float32   `json:"weight"`
	Reps         int       `json:"reps"`
	Date         time.Time `json:"date"`
	Estimated1RM float32   `json:"estimated_1rm"`
}
