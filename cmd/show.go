package cmd

import (
	"fmt"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var showSessionCmd = &cobra.Command{
	Use:   "show-session",
	Short: "Show current session status",
	RunE: func(cmd *cobra.Command, args []string) error {
		if !utils.SessionExists() {
			return fmt.Errorf("No active session")
		}

		state, err := utils.LoadSessionState()
		if err != nil {
			return fmt.Errorf("Failed to load session: %w", err)
		}

		duration := time.Since(state.StartTime).Round(time.Second)

		// Print header.
		cyan := color.New(color.FgCyan).SprintFunc()
		yellow := color.New(color.FgYellow).SprintFunc()
		red := color.New(color.FgRed).SprintFunc()
		green := color.New(color.FgGreen).SprintFunc()

		fmt.Printf("%s\n", green(state.ProgramName))
		fmt.Printf("\n%s %s\n", red("Session:"), state.ProgramBlockName)
		fmt.Printf("%s %s\n", cyan("Description:"), state.ProgramBlockDescription)
		fmt.Printf("%s %s\n\n", red("Duration:"), duration)

		for exIdx, exercise := range state.Exercises {
			ex := exercise.Exercise
			fmt.Printf("%s %s\n", cyan(fmt.Sprintf("%d.", exIdx+1)), yellow(ex.Name))

			// Exercise metadata.
			if !ex.LastPerformed.IsZero() {
				fmt.Printf("   %s %s\n",
					cyan("Last performed:"),
					ex.LastPerformed.Format("2006-01-02"))
			}

			if ex.BestSet != nil {
				fmt.Printf("   %s %.1fkg × %d (1RM: %.1fkg)\n",
					cyan("All-time PR:"),
					ex.BestSet.Weight,
					ex.BestSet.Reps,
					utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps))
			}

			if exercise.ProgramNotes != "" {
				fmt.Printf("   %s %s\n", cyan("Program Notes:"), exercise.ProgramNotes)
			}

			if exercise.SessionNotes != "" {
				fmt.Printf("   %s %s\n", green("Session Notes:"), exercise.SessionNotes)
			}

			// Table header.
			fmt.Println("\n   ┌──────────┬───────────────┬─────────────────┬───────────────┐")
			fmt.Println("   │  Set     │ Target Reps   │ Current         │ Prev Session  │")
			fmt.Println("   ├──────────┼───────────────┼─────────────────┼───────────────┤")

			for setIdx, set := range exercise.Sets {
				var prevSet string

				// Get previous set if exists.
				if setIdx < len(exercise.PreviousSets) {
					ps := exercise.PreviousSets[setIdx]
					prevSet = fmt.Sprintf("%.1fkg × %d", ps.Weight, ps.Reps)
				} else {
					prevSet = "N/A"
				}

				// Format current set.
				setStr := fmt.Sprintf("%.1fkg × %d", set.Weight, set.Reps)

				// Compare new 1RM vs old best 1RM.
				existing1RM := float32(0)
				if ex.BestSet != nil {
					existing1RM = utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps)
				}

				current1RM := utils.CalculateEpley1RM(set.Weight, set.Reps)

				if set.Weight > 0 && current1RM > existing1RM {
					setStr += " ★"
				}

				if set.Weight == 0 {
					setStr = "Not completed"
				}

				targetRep := exercise.TargetReps[setIdx]

				fmt.Printf("   │ %-8d │ %-13s │ %-15s │ %-13s │\n", setIdx+1, targetRep, setStr, prevSet)
			}

			fmt.Println("   └──────────┴───────────────┴─────────────────┴───────────────┘")
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(showSessionCmd)
}
