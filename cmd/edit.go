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
	setWeight    float32
	setReps      int
	isBW         bool
	setIndexFlag int // optional flag: if > 0, update that specific set (1-indexed)
)

var editSetCmd = &cobra.Command{
	Use:   "edit-set [exercise-index]",
	Short: "Edit a set in the current session. If --set-index is provided, update that set; otherwise, update the next unfilled set in order.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		exerciseIndex, err := strconv.Atoi(args[0])
		if err != nil || exerciseIndex < 1 {
			return fmt.Errorf("Invalid exercise index")
		}
		exerciseIndex--

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}
		if exerciseIndex >= len(state.Exercises) {
			return fmt.Errorf("Exercise index out of range")
		}

		exercise := &state.Exercises[exerciseIndex]

		// If the user marks the set as bodyweight, force weight to 0.
		if isBW {
			setWeight = 0
		}

		var indexToUpdate int
		if setIndexFlag > 0 {
			indexToUpdate = setIndexFlag - 1
			if indexToUpdate < 0 || indexToUpdate >= len(exercise.Sets) {
				return fmt.Errorf("Set index out of range")
			}
		} else {
			// Find the first set that is “empty” (weight and reps are zero).
			found := false
			for i, set := range exercise.Sets {
				if set.Weight == 0 && set.Reps == 0 {
					indexToUpdate = i
					found = true
					break
				}
			}
			if !found {
				return fmt.Errorf("All sets are filled; specify a set index to update")
			}
		}

		// Update the selected set.
		exercise.Sets[indexToUpdate] = models.ExerciseSet{
			ID:         uuid.New().String(),
			Weight:     setWeight,
			Reps:       setReps,
			Timestamp:  time.Now().UTC(),
			Bodyweight: isBW,
		}

		// Save the updated session.
		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Println("✅ Set updated successfully")
		return nil
	},
}

func init() {
	// Register flags.
	editSetCmd.Flags().Float32VarP(&setWeight, "weight", "w", 0, "Weight used")
	editSetCmd.Flags().IntVarP(&setReps, "reps", "r", 0, "Reps performed")
	editSetCmd.Flags().BoolVarP(&isBW, "bodyweight", "b", false, "Mark the set as bodyweight (ignores -w)")
	editSetCmd.Flags().IntVarP(&setIndexFlag, "set-index", "s", 0, "Optional set index (1-indexed) to update. If not provided, updates the next unfilled set.")

	editSetCmd.MarkFlagRequired("reps")

	rootCmd.AddCommand(editSetCmd)
}
