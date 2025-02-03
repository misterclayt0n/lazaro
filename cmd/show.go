package cmd

import (
	"fmt"
	"strings"
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

		// Define color functions.
		cyan := color.New(color.FgCyan).SprintFunc()
		yellow := color.New(color.FgYellow).SprintFunc()
		red := color.New(color.FgRed).SprintFunc()
		green := color.New(color.FgGreen).SprintFunc()

		// Print header info.
		fmt.Printf("%s\n", green(state.ProgramName))
		fmt.Printf("\n%s %s\n", red("Session:"), state.ProgramBlockName)
		fmt.Printf("%s %s\n", cyan("Description:"), state.ProgramBlockDescription)
		fmt.Printf("%s %s\n\n", red("Duration:"), duration)

		// Define table indent and column widths.
		tableIndent := "   "
		setColWidth := 6
		targetColWidth := 20
		currentColWidth := 20
		prevColWidth := 15

		// Build horizontal borders and header lines with no extra padding.
		horizontalBorder := tableIndent + "┌" +
			strings.Repeat("─", setColWidth) + "┬" +
			strings.Repeat("─", targetColWidth) + "┬" +
			strings.Repeat("─", currentColWidth) + "┬" +
			strings.Repeat("─", prevColWidth) + "┐"
		headerLine := fmt.Sprintf(tableIndent+"│%-*s│%-*s│%-*s│%-*s│",
			setColWidth, "Set",
			targetColWidth, "Target",
			currentColWidth, "Current",
			prevColWidth, "Prev Session",
		)
		midBorder := tableIndent + "├" +
			strings.Repeat("─", setColWidth) + "┼" +
			strings.Repeat("─", targetColWidth) + "┼" +
			strings.Repeat("─", currentColWidth) + "┼" +
			strings.Repeat("─", prevColWidth) + "┤"
		bottomBorder := tableIndent + "└" +
			strings.Repeat("─", setColWidth) + "┴" +
			strings.Repeat("─", targetColWidth) + "┴" +
			strings.Repeat("─", currentColWidth) + "┴" +
			strings.Repeat("─", prevColWidth) + "┘"

		// Loop over each exercise.
		for exIdx, exercise := range state.Exercises {
			ex := exercise.Exercise
			fmt.Printf("%s %s\n", cyan(fmt.Sprintf("%d.", exIdx+1)), yellow(ex.Name))

			// Optional metadata.
			if !ex.LastPerformed.IsZero() {
				fmt.Printf("   %s %s\n", cyan("Last performed:"), ex.LastPerformed.Format("2006-01-02"))
			}
			if ex.BestSet != nil {
				fmt.Printf("   %s %.1fkg × %d (1RM: %.1fkg)\n",
					cyan("All-time PR:"), ex.BestSet.Weight, ex.BestSet.Reps,
					utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps))
			}
			if exercise.ProgramNotes != "" {
				fmt.Printf("   %s %s\n", cyan("Program Notes:"), exercise.ProgramNotes)
			}
			if exercise.SessionNotes != "" {
				fmt.Printf("   %s %s\n", green("Session Notes:"), exercise.SessionNotes)
			}

			// Print table header.
			fmt.Println(horizontalBorder)
			fmt.Println(headerLine)
			fmt.Println(midBorder)

			// Print each set.
			for setIdx, set := range exercise.Sets {
				// Build previous set string.
				var prevSet string
				if setIdx < len(exercise.PreviousSets) {
					ps := exercise.PreviousSets[setIdx]
					prevSet = fmt.Sprintf("%.1fkg × %d", ps.Weight, ps.Reps)
				} else {
					prevSet = "N/A"
				}

				// Build current set string.
				var setStr string
				if set.Weight == 0 {
					setStr = "Not completed"
				} else {
					setStr = fmt.Sprintf("%.1fkg × %d", set.Weight, set.Reps)
					existing1RM := float32(0)
					if ex.BestSet != nil {
						existing1RM = utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps)
					}
					current1RM := utils.CalculateEpley1RM(set.Weight, set.Reps)
					if current1RM > existing1RM {
						setStr += " ★"
					}
				}

				// Build target string.
				targetRep := ""
				if setIdx < len(exercise.TargetReps) {
					targetRep = exercise.TargetReps[setIdx]
				}
				var targetWeight string
				if exercise.TargetRMPercent != nil && exercise.Program1RM != nil {
					calculated := *exercise.Program1RM * (*exercise.TargetRMPercent / 100)
					targetWeight = fmt.Sprintf("(%.1fkg)", calculated)
				}
				// Append target RPE and RM% info.
				if exercise.TargetRPE != nil || exercise.TargetRMPercent != nil {
					var parts []string
					if exercise.TargetRPE != nil {
						parts = append(parts, fmt.Sprintf("@%.1f", *exercise.TargetRPE))
					}
					if exercise.TargetRMPercent != nil {
						rmPart := fmt.Sprintf("@%.0f%%", *exercise.TargetRMPercent)
						if targetWeight != "" {
							rmPart += " " + targetWeight
						}
						parts = append(parts, rmPart)
					}
					if len(parts) > 0 {
						targetRep += " " + strings.Join(parts, "/")
					}
				}

				// Print the row using no extra padding beyond the fixed width.
				fmt.Printf(tableIndent+"│%-*d│%-*s│%-*s│%-*s│\n",
					setColWidth, setIdx+1,
					targetColWidth, targetRep,
					currentColWidth, setStr,
					prevColWidth, prevSet,
				)
			}
			fmt.Println(bottomBorder)
			fmt.Println() // extra blank line between exercises
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(showSessionCmd)
}
