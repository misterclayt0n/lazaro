package models

import "time"

type Exercise struct {
	ID             string           `json:"id"`
	Name           string           `json:"name"`
	Description    string           `json:"description"`
	PrimaryMuscle  string           `json:"primary_muscle"`
	CreatedAt      time.Time        `json:"created_at"`
	PRHistory      []PersonalRecord `json:"pr_history"`
	CurrentPR      *PersonalRecord  `json:"current_pr"`
	EstimatedOneRM float32          `json:"estimated_one_rm"`
	BestSet        *ExerciseSet     `json:"best_set"` // All-time best set for 1RM display.
	LastPerformed  time.Time        `json:"last_performed"`
}

type SessionExercise struct {
	ID         string          `json:"id"`
	Exercise   Exercise        `json:"exercise"`
	Sets       []ExerciseSet   `json:"sets"`
	Notes      string          `json:"notes"`
	PreviousPR *PersonalRecord `json:"previous_pr"`
    PreviousSets []ExerciseSet `json:"previous_sets"` // Sets from last session.
}

type ExerciseSet struct {
	ID              string    `json:"id"`
	Weight          float32   `json:"weight"`
	Reps            int       `json:"reps"`
	TargetRPE       *float32  `json:"target_rpe,omitempty"`
	TargetRMPercent *float32  `json:"target_rm_percent,omitempty"`
	Notes           string    `json:"notes"`
	Timestamp       time.Time `json:"timestamp"`
}

type ProgramExercise struct {
	ID              string   `json:"id"`
	ExerciseID      string   `json:"exercise_id"`
	Sets            int      `json:"sets"`
	Reps            string   `json:"reps"`
	TargetRPE       *float32 `json:"target_rpe,omitempty"`
	TargetRMPercent *float32 `json:"target_rm_percent,omitempty"`
	Notes           string   `json:"notes,omitempty"`
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
