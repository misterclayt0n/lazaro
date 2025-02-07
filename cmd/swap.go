package cmd

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var swapExerciseCmd = &cobra.Command{
	Use:   "swap-ex [exercise-index] [new-exercise-name]",
	Short: "Swap an exercise in the current session with another",
	Args:  cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		// Load session state.
		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}
		idx, err := strconv.Atoi(args[0])
		if err != nil || idx < 1 || idx > len(state.Exercises) {
			return fmt.Errorf("Invalid exercise index")
		}
		exercise := &state.Exercises[idx-1]

		// For this example, assume the allowed options are stored in a field in the SessionExercise.
		allowedOptions := exercise.Options

		if len(allowedOptions) == 0 {
			return fmt.Errorf("No variations available for this exercise")
		}

		// Determine the chosen option (by name or index).
		optionArg := args[1]
		var chosenOption string
		// For example, if the user supplies a number, treat it as an index:
		if optionIdx, err := strconv.Atoi(optionArg); err == nil {
			if optionIdx < 1 || optionIdx > len(allowedOptions) {
				return fmt.Errorf("Invalid variation index")
			}
			chosenOption = allowedOptions[optionIdx-1]
		} else {
			// Otherwise, assume it's a name; check that it's allowed (case-insensitively)
			for _, opt := range allowedOptions {
				if strings.EqualFold(opt, optionArg) {
					chosenOption = opt
					break
				}
			}
			if chosenOption == "" {
				return fmt.Errorf("Option '%s' is not one of the allowed variations", optionArg)
			}
		}

		// Now look up the exercise record corresponding to chosenOption.
		st := storage.NewStorage()
		newExercise, err := st.GetExerciseByName(chosenOption)
		if err != nil {
			return fmt.Errorf("Failed to get exercise '%s': %w", chosenOption, err)
		}

		// Update the session's exercise.
		exercise.Exercise = *newExercise

		// NOTE: Here is where I update the data from the new exercise.
		// Clear the 1RM value so that no calculations are performed
		exercise.Program1RM = nil // TODO: Change this behavior to calculate via the estimate 1RM for the exercise

		// Load previous sets for the swap exercise.
		newPrevSession, err := st.GetPreviousSession(newExercise.ID, state.ProgramBlockID)
		if err != nil {
			return fmt.Errorf("Failed to get previous session for the new exercise: %w", err)
		}
		var newPrevSets []models.ExerciseSet
		if newPrevSession != nil {
			newPrevSets = getSetsForExercise(newPrevSession, newExercise.ID)
		}

		// Align the number of previous sets with the number of sets in this session.
		exercise.PreviousSets = utils.AlignPreviousSets(newPrevSets, len(exercise.Sets))

		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Printf("✅ Swapped exercise to variation '%s'\n", chosenOption)
		return nil
	},
}

func init() {
	rootCmd.AddCommand(swapExerciseCmd)
}
