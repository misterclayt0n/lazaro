package cmd

import (
	"fmt"

	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var programName string

var startCmd = &cobra.Command{
	Use:   "start-session",
	Short: "Starts a new training session",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		sessionID, err := st.StartSession(programName)

		if err != nil {
			return fmt.Errorf("Failed to start session: %w", err)
		}

		fmt.Printf("✅ Started session %s\n", sessionID)
		return nil
	},
}

func init() {
	// Registers the command as a subcommand of rootCmd.
	rootCmd.AddCommand(startCmd)

	// Define flags.
	startCmd.Flags().StringVarP(&programName, "program", "p", "", "Program name/ID")
	startCmd.MarkFlagRequired("program")
}
