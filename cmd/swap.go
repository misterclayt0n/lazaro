package cmd

import (
	"fmt"
	"strconv"
	"time"

	"github.com/google/uuid"
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
		if !utils.SessionExists() {
			return fmt.Errorf("No active session currently")
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

		newExerciseName := args[1]

		st := storage.NewStorage()
		newExercise, err := st.GetExerciseByName(newExerciseName)
		if err != nil {
			return fmt.Errorf("Failed to find exercise %s: %w", newExerciseName, err)
		}

		prevSession, err := st.GetPreviousSession(newExercise.ID, state.ProgramBlockID)
		if err != nil {
			return fmt.Errorf("Failed to get previous session for new exercise: %w", err)
		}

		var prevSets []models.ExerciseSet
		if prevSession != nil {
			prevSets, err = st.GetExerciseSetsForSession(prevSession.ID, newExercise.ID)

			if err != nil {
				return fmt.Errorf("Failed to get previous sets: %w", err)
			}
		}

		currentSetCount := len(state.Exercises[exerciseIndex].Sets)
		alignedPrevSets := utils.AlignPreviousSets(prevSets, currentSetCount)

		newSets := make([]models.ExerciseSet, currentSetCount)
		for i := range newSets {
			newSets[i] = models.ExerciseSet{
				ID:        uuid.New().String(),
				Weight:    0,
				Reps:      0,
				Timestamp: time.Now().UTC(),
			}
		}

		sessionExercise := &state.Exercises[exerciseIndex]
		sessionExercise.Exercise = *newExercise
		sessionExercise.PreviousSets = alignedPrevSets
		sessionExercise.Sets = newSets

		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Printf("✅ Swapped exercise to %s\n", newExercise.Name)
		return nil
	},
}

func init() {
	rootCmd.AddCommand(swapExerciseCmd)
}
