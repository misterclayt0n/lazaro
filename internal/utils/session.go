package utils

import (
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func getSessionPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}

	dir := filepath.Join(home, ".config", "lazaro")
	os.MkdirAll(dir, 0755)
	return filepath.Join(dir, "current_session.toml"), nil
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
	return os.Remove(path)
}

func SessionExists() bool {
	path, err := getSessionPath()
	if err != nil {
		return false
	}
	_, err = os.Stat(path)
	return !os.IsNotExist(err)
}
