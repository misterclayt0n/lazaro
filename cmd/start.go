package cmd

import (
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var (
	programName string
	blockName   string
)

var startCmd = &cobra.Command{
	Use:   "start-session",
	Short: "Starts a new training session",
	RunE: func(cmd *cobra.Command, args []string) error {
		if utils.SessionExists() {
			return fmt.Errorf("A session is already in progress...")
		}

		st := storage.NewStorage()

		// Get program and validate block exists.
		program, err := st.GetProgramByName(programName)
		if err != nil {
			return fmt.Errorf("Failed to get program: %w", err)
		}

		// Find the specific block in the program.
		var selectedBlock *models.ProgramBlock
		for _, block := range program.Blocks {
			if block.Name == blockName {
				selectedBlock = &block
				break
			}
		}
		if selectedBlock == nil {
			return fmt.Errorf("block '%s' not found in program", blockName)
		}

		// Create session state with correct block ID.
		state := &models.SessionState{
			SessionID:      uuid.New().String(),
			ProgramBlockID: selectedBlock.ID,
			StartTime:      time.Now().UTC(),
		}

		// Initialize exercises only from the selected block.
		for _, pe := range selectedBlock.Exercises {
			exercise, err := st.GetExerciseByID(pe.ExerciseID)
			if err != nil {
				return fmt.Errorf("Failed to get exercise: %w", err)
			}

			state.Exercises = append(state.Exercises, models.SessionExercise{
				ID:       uuid.New().String(),
				Exercise: *exercise,
				Sets:     make([]models.ExerciseSet, pe.Sets),
				Notes:    pe.Notes,
			})
		}

		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Printf("✅ Started session %s for block '%s'\n", state.SessionID, blockName)
		return nil
	},
}

func init() {
	rootCmd.AddCommand(startCmd)

	startCmd.Flags().StringVarP(&programName, "program", "p", "", "Program name")
	startCmd.Flags().StringVarP(&blockName, "block", "b", "", "Program block name")
	startCmd.MarkFlagRequired("program")
	startCmd.MarkFlagRequired("block")
}
