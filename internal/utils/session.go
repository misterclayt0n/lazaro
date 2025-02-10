package utils

import (
	"os"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func getSessionPath() (string, error) {
	return "current_session.toml", nil // I have removed the whole config thing, so fuck it
}

func SaveSessionState(state *models.SessionState) error {
	path, err := getSessionPath()
	if err != nil {
		return err
	}

	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	return toml.NewEncoder(f).Encode(state)
}

func LoadSessionState() (*models.SessionState, error) {
	path, err := getSessionPath()
	if err != nil {
		return nil, err
	}

	var state models.SessionState
	_, err = toml.DecodeFile(path, &state)
	if err != nil {
		return nil, err
	}

	return &state, nil
}

func ClearSessionState() error {
	path, err := getSessionPath()
	if err != nil {
		return err
	}

	// Ensure file exists before trying to remove.
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return nil
	}

	return os.Remove(path)
}

func SessionExists() bool {
	path, err := getSessionPath()
	if err != nil {
		return false
	}

	// Check both file existence and non-empty content.
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}

	// Verify the file actually contains a valid session.
	state, err := LoadSessionState()
	if err != nil || state.SessionID == "" || len(state.Exercises) == 0 {
		_ = ClearSessionState()
		return false
	}

	return true
}

func AlignPreviousSets(prevSets []models.ExerciseSet, requiredSets int) []models.ExerciseSet {
	aligned := make([]models.ExerciseSet, requiredSets)
	for i := 0; i < requiredSets; i++ {
		if i < len(prevSets) {
			aligned[i] = prevSets[i]
		} else {
			aligned[i] = models.ExerciseSet{Weight: 0, Reps: 0}
		}
	}
	return aligned
}
