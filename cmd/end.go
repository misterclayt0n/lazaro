package cmd

import (
	"fmt"

	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var endSessionCmd = &cobra.Command{
	Use:   "end-session",
	Short: "End the current training session",
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}

		st := storage.NewStorage()

		// Save to database.
		if err := st.SaveSession(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		// Clear temp file.
		if err := utils.ClearSessionState(); err != nil {
			return fmt.Errorf("Failed to clear session: %w", err)
		}

		fmt.Println("✅ Session saved successfully")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(endSessionCmd)
}
