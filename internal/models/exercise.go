package models

import "time"

type Exercise struct {
	ID             string           `toml:"id"`
	Name           string           `toml:"name"`
	Description    string           `toml:"description"`
	PrimaryMuscle  string           `toml:"primary_muscle"`
	CreatedAt      time.Time        `toml:"created_at"`
	PRHistory      []PersonalRecord `toml:"pr_history"`
	CurrentPR      *PersonalRecord  `toml:"current_pr"`
	EstimatedOneRM float32          `toml:"estimated_one_rm"`
	BestSet        *ExerciseSet     `toml:"best_set"`
	LastPerformed  time.Time        `toml:"last_performed"`
}

type SessionExercise struct {
	ID              string          `toml:"id"`
	Exercise        Exercise        `toml:"exercise"`
	Sets            []ExerciseSet   `toml:"sets"`
	ProgramNotes    string          `toml:"notes"`         // From ProgramExercise.
	SessionNotes    string          `toml:"session_notes"` // User input (like "felt like shit").
	PreviousPR      *PersonalRecord `toml:"previous_pr"`
	PreviousSets    []ExerciseSet   `toml:"previous_sets"`
	TargetReps      []string        `toml:"target_reps"`
	TargetRPE       []float32       `toml:"target_rpe,omitempty"`
	TargetRMPercent []float32       `toml:"target_rm_percent,omitempty"`
	Program1RM      *float32        `toml:"program_1rm,omitempty"`
}

type ExerciseSet struct {
	ID              string    `toml:"id"`
	Weight          float32   `toml:"weight"`
	Reps            int       `toml:"reps"`
	TargetRPE       *float32  `toml:"target_rpe,omitempty"`
	TargetRMPercent *float32  `toml:"target_rm_percent,omitempty"`
	Notes           string    `toml:"notes"`
	Timestamp       time.Time `toml:"timestamp"`
}

type ProgramExercise struct {
	ID              string    `json:"id"`
	ExerciseID      string    `json:"exercise_id"`
	Sets            int       `json:"sets"`
	Reps            []string  `json:"reps"`
	TargetRPE       []float32 `json:"target_rpe,omitempty"`
	TargetRMPercent []float32 `json:"target_rm_percent,omitempty"`
	ProgramNotes    string    `json:"program_notes,omitempty"`
	Program1RM      *float32  `json:"program_1rm,omitempty"`
}

//
// For TOML parsing only
//

type ExerciseDefTOML struct {
	Name          string  `toml:"name"`
	Description   string  `toml:"description"`
	PrimaryMuscle string  `toml:"primary_muscle"`
	Estimate1RM   float32 `toml:"estimate_1rm"`
}

type ExerciseImport struct {
	Exercises []ExerciseDefTOML `toml:"exercise"`
}
