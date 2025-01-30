package cmd

import (
	"fmt"
	"time"

	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var showSessionCmd = &cobra.Command{
	Use:   "show-session",
	Short: "Show current session status",
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}

		duration := time.Since(state.StartTime).Round(time.Second)

		fmt.Printf("Session ID: %s\n", state.SessionID)
		fmt.Printf("Duration:   %s\n", duration)
		fmt.Println("\nExercises:")

		for exIdx, exercise := range state.Exercises {
			fmt.Printf("\n%d. %s\n", exIdx+1, exercise.Exercise.Name)
			for setIdx, set := range exercise.Sets {
				if set.Weight > 0 {
					fmt.Printf("   Set %d: %.1fkg x %d\n", setIdx+1, set.Weight, set.Reps)
				} else {
					fmt.Printf("   Set %d: Not completed\n", setIdx+1)
				}
			}
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(showSessionCmd)
}
