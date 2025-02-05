package cmd

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/config"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var (
	initDB bool // new flag to trigger DB initialization
)

var initSetupCmd = &cobra.Command{
	Use:   "init-config",
	Short: "Set up persistent configuration for Lazaro and optionally initialize the database",
	RunE: func(cmd *cobra.Command, args []string) error {
		// If --init-db is passed, load the config and initialize the database tables.
		if initDB {
			cfg, err := config.LoadConfig()
			if err != nil {
				return fmt.Errorf("failed to load configuration: %w", err)
			}
			db, err := sql.Open("libsql", cfg.DB.ConnectionString)
			if err != nil {
				return fmt.Errorf("failed to open database: %w", err)
			}
			// Call the exported initialization function.
			if err := storage.InitializeDB(db); err != nil {
				return fmt.Errorf("failed to initialize database: %w", err)
			}
			fmt.Println("✅ Database initialized successfully")
			return nil
		}

		// Otherwise, do the usual configuration setup:
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
	// Add the new flag.
	initSetupCmd.Flags().BoolVar(&initDB, "init-db", false, "Initialize the database tables")
	rootCmd.AddCommand(initSetupCmd)
}
