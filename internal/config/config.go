package config

import (
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
)

type Config struct {
	DB DBConfig `toml:"database"`
}

type DBConfig struct {
	ConnectionString string `toml:"connection_string"` // The entire DB connection string.
}

// Returns the path to the config file.
func GetConfigPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}

	dir := filepath.Join(home, ".config", "lazaro")
	return filepath.Join(dir, "config.toml"), nil
}

// Reads the configuration from the config file
func LoadConfig() (*Config, error) {
	path, err := GetConfigPath()
	if err != nil {
		return nil, err
	}

	var cfg Config
	if _, err := toml.DecodeFile(path, &cfg); err != nil {
		return nil, err
	}

	// Check for a DEV_MODE environment variable.
	if os.Getenv("DEV_MODE") == "true" {
		cfg.DB.ConnectionString = "file:./local.db?cache=shared&mode=rwc"
	}

	return &cfg, nil
}
