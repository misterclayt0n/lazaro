package cmd

import (
	"fmt"

	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var gitPushCmd = &cobra.Command{
	Use:   "export [output-file]",
	Short: "Export all the database data to a TOML file",
	RunE: func(cmd *cobra.Command, args []string) error {
		outputFile := "db_dump.toml" // Default filename.
		if len(args) == 1 {
			outputFile = args[0]
		}

		if err := storage.ExportDBToTOML(outputFile); err != nil {
			return fmt.Errorf("error exporting database: %w", err)
		}

		fmt.Printf("✅ Database exported successfully to %s\n", outputFile)
		return nil
	},
}

var buildDBCmd = &cobra.Command{
	Use:   "build-db [dump-file]",
	Short: "Build the entire database from the given TOML dump file",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		dumpFile := args[0]
		if err := storage.ImportDBFromTOML(dumpFile); err != nil {
			return fmt.Errorf("Failed to build database: %w", err)
		}
		fmt.Println("✅ Database built successfully from TOML dump.")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(gitPushCmd)
	rootCmd.AddCommand(buildDBCmd)
}
