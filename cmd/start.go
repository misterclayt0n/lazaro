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
			return fmt.Errorf("Block '%s' not found in program", blockName)
		}

		// Create session state with correct block ID.
		state := &models.SessionState{
			SessionID:               uuid.New().String(),
			ProgramName:             program.Name,
			ProgramBlockID:          selectedBlock.ID,
			ProgramBlockDescription: selectedBlock.Description,
			ProgramBlockName:        selectedBlock.Name,
			StartTime:               time.Now().UTC(),
		}

		if len(selectedBlock.Exercises) == 0 {
			return fmt.Errorf("Block '%s' has no exercises", blockName)
		}

		// Initialize exercises only from the selected block.
		for _, pe := range selectedBlock.Exercises {
			exercise, err := st.GetExerciseByID(pe.ExerciseID)
			if err != nil {
				return fmt.Errorf("Failed to get exercise: %w", err)
			}

			// Load previous session sets.
			prevSession, err := st.GetPreviousSession(exercise.ID, selectedBlock.ID)
			if err != nil {
				return fmt.Errorf("Failed to get previous session: %w", err)
			}

			var prevSets []models.ExerciseSet
			if prevSession != nil {
				prevSets = getSetsForExercise(prevSession, exercise.ID)
			}

			targetReps := make([]string, pe.Sets)
			for i := 0; i < pe.Sets; i++ {
				if i < len(pe.Reps) {
					targetReps[i] = pe.Reps[i]
				} else if len(pe.Reps) > 0 {
					// Use last specified rep scheme for remaining sets
					targetReps[i] = pe.Reps[len(pe.Reps)-1]
				} else {
					targetReps[i] = ""
				}
			}
			state.Exercises = append(state.Exercises, models.SessionExercise{
				ID:           uuid.New().String(),
				Exercise:     *exercise,
				Sets:         make([]models.ExerciseSet, pe.Sets),
				PreviousSets: alignPreviousSets(prevSets, pe.Sets),
				ProgramNotes: pe.ProgramNotes,
				// SessionNotes stays empty for now since the user hasn't entered anything yet.
				SessionNotes: "",
				TargetReps:   targetReps,
			})
		}

		if err := utils.SaveSessionState(state); err != nil {
			return fmt.Errorf("Failed to save session: %w", err)
		}

		fmt.Printf("✅ Started session %s for block '%s'\n", state.SessionID, blockName)
		return nil
	},
}

func getSetsForExercise(prevSession *models.TrainingSession, exerciseID string) []models.ExerciseSet {
	st := storage.NewStorage()

	// First, get the training_session_exercises.id record that matches.
	// the previous session's ID and the target exercise ID.
	var sessionExerciseID string
	row := st.DB.QueryRow(
		`SELECT id
		 FROM training_session_exercises
		 WHERE training_session_id = ?
		   AND exercise_id = ?`,
		prevSession.ID,
		exerciseID,
	)
	if err := row.Scan(&sessionExerciseID); err != nil {
		// If not found or error, return an empty slice.
		return nil
	}

	rows, err := st.DB.Query(`
        SELECT id, weight, reps, timestamp
        FROM exercise_sets
        WHERE session_exercise_id = ?
        ORDER BY timestamp ASC
    `, sessionExerciseID)
	if err != nil {
		return nil
	}
	defer rows.Close()

	// Use an unbounded slice.
	var sets []models.ExerciseSet

	for rows.Next() {
		var (
			id      string
			weight  float32
			reps    int
			rawTime string
		)
		if err := rows.Scan(&id, &weight, &reps, &rawTime); err != nil {
			continue
		}
		ts, _ := time.Parse(time.RFC3339, rawTime)
		sets = append(sets, models.ExerciseSet{
			ID:        id,
			Weight:    weight,
			Reps:      reps,
			Timestamp: ts,
		})
	}

	return sets
}

func alignPreviousSets(prevSets []models.ExerciseSet, requiredSets int) []models.ExerciseSet {
	aligned := make([]models.ExerciseSet, requiredSets)
	for i := 0; i < requiredSets; i++ {
		if i < len(prevSets) {
			aligned[i] = prevSets[i]
		} else {
			aligned[i] = models.ExerciseSet{Weight: 0, Reps: 0} // Mark as N/A.
		}
	}
	return aligned
}

func init() {
	rootCmd.AddCommand(startCmd)

	startCmd.Flags().StringVarP(&programName, "program", "p", "", "Program name")
	startCmd.Flags().StringVarP(&blockName, "block", "b", "", "Program block name")
	startCmd.MarkFlagRequired("program")
	startCmd.MarkFlagRequired("block")
}
