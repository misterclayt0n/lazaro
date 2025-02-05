package models

import "time"

const (
	TechniqueSuperset = "superset"
	TechniqueMyoreps  = "myoreps"
	TechniqueHell     = "hell"
	TechniqueDrop     = "drop"
)

type TrainingSession struct {
	ID        string            `json:"id"`
	Program   Program           `json:"program"`
	StartTime time.Time         `json:"start_time"`
	EndTime   *time.Time        `json:"end_time,omitempty"`
	Exercises []SessionExercise `json:"exercises"`
	Notes     string            `json:"notes"`
}

type PersonalRecord struct {
	ExerciseName string    `json:"exercise_name"`
	Weight       float32   `json:"weight"`
	Reps         int       `json:"reps"`
	Date         time.Time `json:"date"`
	Estimated1RM float32   `json:"estimated_1rm"`
}

type SessionState struct {
	SessionID               string            `toml:"session_id"`
	ProgramBlockID          string            `toml:"program_block_id"`
	ProgramBlockDescription string            `toml:"program_block_description"`
	ProgramName             string            `toml:"program_name"`
	ProgramBlockName        string            `toml:"program_block_name"`
	ExerciseNote            string            `toml:"exercise_note"`
	StartTime               time.Time         `toml:"start_time"`
	Exercises               []SessionExercise `toml:"exercises"`
	CurrentSetID            int               `toml:"current_set_id"`
	Week                    int               `toml:"week,omitempty"`
}

type TempSet struct {
	ExerciseIndex int     `toml:"exercise_index"`
	SetIndex      int     `toml:"set_index"`
	Weight        float32 `toml:"weight"`
	Reps          int     `toml:"reps"`
}
