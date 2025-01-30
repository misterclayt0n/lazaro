package cmd

import (
	"fmt"
	"strconv"
	"time"

	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var (
	setWeight float32
	setReps   int
)

var editSetCmd = &cobra.Command{
	Use:   "edit-set [exercise-index] [set-index]",
	Short: "Edit a set in the current session",
	Args:  cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		// Parse arguments
		exerciseIndex, err := strconv.Atoi(args[0])
		exerciseIndex--
		if err != nil || exerciseIndex < 0 {
			return fmt.Errorf("Invalid exercise index")
		}

		setIndex, err := strconv.Atoi(args[1])
		setIndex--
		if err != nil || setIndex < 0 {
			return fmt.Errorf("Invalid set index")
		}

		// Load session
		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}

		// Validate indices
		if exerciseIndex >= len(state.Exercises) {
			return fmt.Errorf("Exercise index out of range")
		}

		exercise := &state.Exercises[exerciseIndex]
		if setIndex >= len(exercise.Sets) {
			return fmt.Errorf("Set index out of range")
		}

		// Update set
		exercise.Sets[setIndex] = models.ExerciseSet{
			ID:        uuid.New().String(),
			Weight:    setWeight,
			Reps:      setReps,
			Timestamp: time.Now().UTC(),
		}

		// Save changes
		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Println("✅ Set updated successfully")
		return nil
	},
}

func init() {
	editSetCmd.Flags().Float32VarP(&setWeight, "weight", "w", 0, "Weight used")
	editSetCmd.Flags().IntVarP(&setReps, "reps", "r", 0, "Reps performed")
	editSetCmd.MarkFlagRequired("weight")
	editSetCmd.MarkFlagRequired("reps")

	rootCmd.AddCommand(editSetCmd)
}
