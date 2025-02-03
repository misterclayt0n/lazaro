package cmd

import (
	"fmt"

	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var cancelSessionCmd = &cobra.Command {
	Use: "cancel-session",
	Short: "Cancel the current training session without saving any data",
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session to cancel")
		}

		// Just clear the temp session file and we gucci.
		if err := utils.ClearSessionState(); err != nil {
			return fmt.Errorf("Failed to cancel session: %w", err)
		}

		fmt.Println("✅ Session cancelled successfully")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(cancelSessionCmd)
}
