package cmd

import (
	"database/sql"
	"fmt"

	_ "github.com/mattn/go-sqlite3" // required for SQLite
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var initSetupCmd = &cobra.Command{
	Use:   "init",
	Short: "Create the database file lazaro.db",
	RunE: func(cmd *cobra.Command, args []string) error {
		db, err := sql.Open("sqlite3", "file:./lazaro.db?cache=shared&mode=rwc")
		if err != nil {
			return fmt.Errorf("Failed to open database: %w", err)
		}
		defer db.Close()

		if err := storage.InitializeDB(db); err != nil {
			return fmt.Errorf("Failed to initialize database: %w", err)
		}
		fmt.Println("✅ Database initialized successfully as lazaro.db")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(initSetupCmd)
}
