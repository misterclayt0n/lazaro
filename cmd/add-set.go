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
	newSetWeight float32
	newSetReps   int
	newSetBW     bool
)

var addSetCmd = &cobra.Command{
	Use:   "add-set [exercise-index]",
	Short: "Add a new set to an exercise in the current session (no target data)",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		exIdx, err := strconv.Atoi(args[0])
		if err != nil || exIdx < 1 {
			return fmt.Errorf("Invalid exercise index. Must be a positive integer")
		}
		exIdx--

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session state: %w", err)
		}

		if exIdx >= len(state.Exercises) {
			return fmt.Errorf("Exercise index out of range")
		}

		if newSetBW {
			newSetWeight = 0
		}

		newSet := models.ExerciseSet{
			ID:        uuid.New().String(),
			Weight:    newSetWeight,
			Reps:      newSetReps,
			Timestamp: time.Now().UTC(),
			Bodyweight: newSetBW,
		}

		state.Exercises[exIdx].Sets = append(state.Exercises[exIdx].Sets, newSet)

		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session state: %w", err)
		}

		fmt.Printf("✅ Added new set to exercise '%s' in the current session\n", state.Exercises[exIdx].Exercise.Name)
		return nil
	},
}

func init() {
	addSetCmd.Flags().Float32VarP(&newSetWeight, "weight", "w", 0, "Weight used for the new set")
	addSetCmd.Flags().IntVarP(&newSetReps, "reps", "r", 0, "Number of reps performed for the new set")
	addSetCmd.Flags().BoolVarP(&newSetBW, "bodyweight", "b", false, "Mark the set as bodyweight (ignores weight)")
	addSetCmd.MarkFlagRequired("reps")
	rootCmd.AddCommand(addSetCmd)
}
