package cmd

import (
	"fmt"
	"strconv"

	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var noteText string

var setNoteCmd = &cobra.Command{
	Use:   "set-note [exercise-index]",
	Short: "Set a note for a specific exercise in the current session",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		// Check if there is an active session.
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		// Parse the exercise index.
		exIdx, err := strconv.Atoi(args[0])
		if err != nil || exIdx < 1 {
			return fmt.Errorf("Invalid exercise index (should be 1-based)")
		}
		exIdx-- // Convert this motherfucker to zero-based index.

		// Load the current session state.
		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}

		// Validate the exercise index.
		if exIdx >= len(state.Exercises) {
			return fmt.Errorf("Exercise index out of range")
		}

		// Update the note for the given exercise.
		state.Exercises[exIdx].SessionNotes = noteText

		// Save the updated session state.
		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Println("✅ Note set successfully")
		return nil
	},
}

func init() {
	setNoteCmd.Flags().StringVarP(&noteText, "note", "n", "", "Note text to set for the exercise")
	setNoteCmd.MarkFlagRequired("note")
	rootCmd.AddCommand(setNoteCmd)
}
