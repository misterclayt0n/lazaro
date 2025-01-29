package models

import "time"

type Program struct {
	ID          string         `json:"id"`
	Name        string         `json:"name"`
	Description string         `json:"description"`
	CreatedAt   time.Time      `json:"created_at"`
	Blocks      []ProgramBlock `json:"blocks"`
}

type ProgramBlock struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Exercises   []ProgramExercise `json:"exercises"`
}

//
// For TOML parsing only
//

type ProgramTOML struct {
	Name        string      `toml:"name"`
	Description string      `toml:"description"`
	Blocks      []BlockTOML `toml:"block"`
}

type BlockTOML struct {
	Name        string         `toml:"name"`
	Description string         `toml:"description"`
	Exercises   []ExerciseTOML `toml:"exercise"`
}

type ExerciseTOML struct {
	Name            string   `toml:"name"`
	Sets            int      `toml:"sets"`
	Reps            string   `toml:"reps"`
	TargetRPE       *float32 `toml:"target_rpe,omitempty"`
	TargetRMPercent *float32 `toml:"target_rm_percent,omitempty"`
	Notes           string   `toml:"notes,omitempty"`
}
