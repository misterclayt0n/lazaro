package cmd

import (
	"fmt"
	"os"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var (
	exerciseName string
	numSets      int
)

var addExerciseCmd = &cobra.Command{
	Use:   "add-exercise",
	Short: "Create a new exercise from the command line",
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		st := storage.NewStorage()

		exercise, err := st.GetExerciseByName(exerciseName)
		if err != nil {
			return fmt.Errorf("Exercise '%s' not found in database: %w", exerciseName, err)
		}

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session state: %w", err)
		}

		// Try to get previous sets for this exercise (from the last valid session
		// in the current program block). If none is found, prevSets remains nil.
		var prevSets []models.ExerciseSet
		prevSession, err := st.GetValidPreviousSession(exercise.ID, state.ProgramBlockID)
		if err == nil && prevSession != nil {
			prevSets = getSetsForExercise(prevSession, exercise.ID)
		}
		alignedPrevSets := utils.AlignPreviousSets(prevSets, numSets)

		newSets := make([]models.ExerciseSet, numSets)
		targetReps := make([]string, numSets)
		targetRPE := make([]float32, numSets)
		targetRMPercent := make([]float32, numSets)

		newSessionExercise := models.SessionExercise{
			ID:              uuid.New().String(),
			Exercise:        *exercise,
			Sets:            newSets,
			PreviousSets:    alignedPrevSets,
			ProgramNotes:    "",
			SessionNotes:    "",
			TargetReps:      targetReps,
			TargetRPE:       targetRPE,
			TargetRMPercent: targetRMPercent,
		}

		state.Exercises = append(state.Exercises, newSessionExercise)
		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to update session state: %w", err)
		}

		fmt.Printf("✅ Added exercise '%s' with %d sets to the current session\n", exercise.Name, numSets)
		return nil
	},
}

var importExercisesCmd = &cobra.Command{
	Use:   "import-exercises [file]",
	Short: "Import exercises from TOML file",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()
		data, err := os.ReadFile(args[0])
		if err != nil {
			return fmt.Errorf("failed to read file: %w", err)
		}

		var importData models.ExerciseImport
		if err := toml.Unmarshal(data, &importData); err != nil {
			return fmt.Errorf("invalid TOML format: %w", err)
		}

		for _, exTOML := range importData.Exercises {
			ex := models.Exercise{
				ID:             uuid.New().String(),
				Name:           exTOML.Name,
				Description:    exTOML.Description,
				PrimaryMuscle:  exTOML.PrimaryMuscle,
				CreatedAt:      time.Now().UTC(),
				EstimatedOneRM: utils.CalculateInitialOneRM(),
			}

			if err := st.CreateExercise(ex); err != nil {
				return fmt.Errorf("Failed to create exercise %s: %w", ex.Name, err)
			}
		}

		fmt.Printf("✅ Imported %d exercises\n", len(importData.Exercises))
		return nil
	},
}

func init() {
	addExerciseCmd.Flags().StringVarP(&exerciseName, "name", "n", "", "Exercise name (must already exist in the database)")
	addExerciseCmd.Flags().IntVarP(&numSets, "sets", "s", 0, "Number of sets for this exercise")
	addExerciseCmd.MarkFlagRequired("name")
	addExerciseCmd.MarkFlagRequired("sets")

	rootCmd.AddCommand(addExerciseCmd)
	rootCmd.AddCommand(importExercisesCmd)
}
