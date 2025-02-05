package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/config"
	"github.com/spf13/cobra"
)

var initSetupCmd = &cobra.Command{
	Use:   "init-config",
	Short: "Set up persistent configuration for Lazaro",
	RunE: func(cmd *cobra.Command, args []string) error {
		var connString string
		fmt.Print("Enter database connection string (e.g. libsql://[DATABASE].turso.io?authToken=[TOKEN]): ")
		fmt.Scanln(&connString)

		cfg := config.Config{
			DB: config.DBConfig{
				ConnectionString: connString,
			},
		}

		path, err := config.GetConfigPath()
		if err != nil {
			return err
		}

		// Ensure the configuration directory exists.
		dir := filepath.Dir(path)
		if err := os.MkdirAll(dir, 0755); err != nil {
			return err
		}

		f, err := os.Create(path)
		if err != nil {
			return err
		}
		defer f.Close()

		if err := toml.NewEncoder(f).Encode(cfg); err != nil {
			return err
		}

		fmt.Println("✅ Configuration saved successfully at", path)

		return nil
	},
}

func init() {
	rootCmd.AddCommand(initSetupCmd)
}
