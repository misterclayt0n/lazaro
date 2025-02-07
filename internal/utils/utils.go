package utils

import (
	"os"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func ParseProgramFromTOML(path string) (*models.Program, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var program models.Program
	if err := toml.Unmarshal(data, &program); err != nil {
		return nil, err
	}

	return &program, nil
}

func BoolToInt(b bool) int {
    if b {
        return 1
    }
    return 0
}
