package cmd

import (
	"fmt"
	"os"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var (
	exerciseName        string
	exerciseDesc        string
	exerciseMuscle      string
	exerciseEstimate1RM float32
)

var addExerciseCmd = &cobra.Command{
	Use:   "add-exercise",
	Short: "Create a new exercise",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		exercise := models.Exercise{
			ID:             uuid.New().String(),
			Name:           exerciseName,
			Description:    exerciseDesc,
			PrimaryMuscle:  exerciseMuscle,
			CreatedAt:      time.Now().UTC(),
			EstimatedOneRM: exerciseEstimate1RM,
		}

		if err := st.CreateExercise(exercise); err != nil {
			return fmt.Errorf("Failed to create exercise: %w", err)
		}

		fmt.Printf("✅ Created exercise: %s\n", exercise.Name)
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
				EstimatedOneRM: exTOML.Estimate1RM,
			}

			if err := st.CreateExercise(ex); err != nil {
				return fmt.Errorf("failed to create exercise %s: %w", ex.Name, err)
			}
		}

		fmt.Printf("✅ Imported %d exercises\n", len(importData.Exercises))
		return nil
	},
}

func init() {
	addExerciseCmd.Flags().StringVarP(&exerciseName, "name", "n", "", "Exercise name")
	addExerciseCmd.Flags().StringVarP(&exerciseDesc, "description", "d", "", "Exercise description")
	addExerciseCmd.Flags().StringVarP(&exerciseMuscle, "muscle", "m", "", "Primary muscle group")
	addExerciseCmd.Flags().Float32Var(&exerciseEstimate1RM, "estimate-1rm", 0, "Estimated 1RM")

	addExerciseCmd.MarkFlagRequired("name")
	addExerciseCmd.MarkFlagRequired("muscle")

	rootCmd.AddCommand(addExerciseCmd)
	rootCmd.AddCommand(importExercisesCmd)
}
