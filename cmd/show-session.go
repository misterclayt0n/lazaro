package cmd

import (
	"fmt"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/models"
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
		if state.Week != 0 {
			fmt.Printf("%s %d\n", yellow("Week:"), state.Week)
		}
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

		// Instead of a simple loop, use an index to allow grouping.
		exCounter := 1
		for i := 0; i < len(state.Exercises); {
			se := state.Exercises[i]
			// If this exercise is part of a superset, group consecutive exercises with Technique "superset"
			if strings.EqualFold(se.Technique, models.TechniqueSuperset) {
				groupIndices := []int{i}
				j := i + 1
				for j < len(state.Exercises) {
					next := state.Exercises[j]
					if strings.EqualFold(next.Technique, models.TechniqueSuperset) &&
						next.TechniqueGroup == se.TechniqueGroup {
						groupIndices = append(groupIndices, j)
						j++
					} else {
						break
					}
				}
				// Print each exercise in the superset with its own number.
				for _, idx := range groupIndices {
					printExerciseDetailsWithIndex := printExerciseDetailsWithIndex // alias if needed
					printExerciseDetailsWithIndex(state.Exercises[idx], exCounter, tableIndent, setColWidth, targetColWidth, currentColWidth, prevColWidth, horizontalBorder, headerLine, midBorder, bottomBorder, cyan, yellow, red, green)
					exCounter++
				}
				i = j // jump to the next non-superset exercise
			} else {
				printExerciseDetailsWithIndex(state.Exercises[i], exCounter, tableIndent, setColWidth, targetColWidth, currentColWidth, prevColWidth, horizontalBorder, headerLine, midBorder, bottomBorder, cyan, yellow, red, green)
				exCounter++
				i++
			}
		}

		return nil
	},
}

func printExerciseDetails(se models.SessionExercise, tableIndent string, setColWidth, targetColWidth, currentColWidth, prevColWidth int, horizontalBorder, headerLine, midBorder, bottomBorder string, cyan, yellow, red, green func(a ...interface{}) string) {
	ex := se.Exercise
	// Print exercise header.
	var technique string
	if se.Technique != "" {
		technique = yellow("(Technique: " + se.Technique + ")")
	} else {
		technique = ""
	}

	fmt.Printf("%s %s\n", cyan("• "+ex.Name), technique)
	// Optional metadata.
	if !ex.LastPerformed.IsZero() {
		fmt.Printf("   %s %s\n", cyan("Last performed:"), ex.LastPerformed.Format("2006-01-02"))
	}
	if ex.BestSet != nil {
		fmt.Printf("   %s %.1fkg × %d (1RM: %.1fkg)\n",
			cyan("All-time PR:"), ex.BestSet.Weight, ex.BestSet.Reps,
			utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps))
	}
	if se.ProgramNotes != "" {
		fmt.Printf("   %s %s\n", cyan("Program Notes:"), se.ProgramNotes)
	}
	if se.SessionNotes != "" {
		fmt.Printf("   %s %s\n", green("Session Notes:"), se.SessionNotes)
	}

	// Print table header.
	fmt.Println(horizontalBorder)
	fmt.Println(headerLine)
	fmt.Println(midBorder)

	for setIdx, set := range se.Sets {
		// Build previous set string.
		var prevSet string
		if setIdx < len(se.PreviousSets) {
			ps := se.PreviousSets[setIdx]
			if ps.Weight == 0 && ps.Reps == 0 {
				prevSet = "First time"
			} else {
				prevSet = fmt.Sprintf("%.1fkg × %d", ps.Weight, ps.Reps)
			}
		} else {
			prevSet = "N/A"
		}

		// Build current set string.
		var setStr string
		if set.Bodyweight {
			setStr = fmt.Sprintf("Bodyweight × %d", set.Reps)
		} else if set.Weight == 0 {
			setStr = "Not completed"
		} else {
			setStr = fmt.Sprintf("%.1fkg × %d", set.Weight, set.Reps)

			existing1RM := float32(0)
			if ex.BestSet != nil {
				existing1RM = utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps)
			}
			current1RM := utils.CalculateEpley1RM(set.Weight, set.Reps)
			if current1RM > existing1RM && !strings.EqualFold(se.Technique, models.TechniqueMyoreps) && !strings.EqualFold(se.Technique, models.TechniqueHell) {
				setStr += " ★"
			}
		}

		// Build target string.
		var targetRep string
		if setIdx < len(se.TargetReps) {
			targetRep = se.TargetReps[setIdx]
		}

		// Append target RPE and RM% info.
		var parts []string
		if setIdx < len(se.TargetRPE) {
			parts = append(parts, fmt.Sprintf("@%.1f", se.TargetRPE[setIdx]))
		}
		if setIdx < len(se.TargetRMPercent) && se.Program1RM != nil {
			calculated := *se.Program1RM * (se.TargetRMPercent[setIdx] / 100)
			parts = append(parts, fmt.Sprintf("@%.0f%% (%.1fkg)", se.TargetRMPercent[setIdx], calculated))
		}
		if len(parts) > 0 {
			targetRep += " " + strings.Join(parts, "/")
		}

		splitRep := strings.SplitN(targetRep, " ", 2)
		repsPart := splitRep[0]
		modifiersPart := ""
		if len(splitRep) > 1 {
			modifiersPart = splitRep[1]
		}

		availableSpace := targetColWidth - len(modifiersPart)
		if availableSpace < 0 {
			availableSpace = 0
		}

		formattedReps := fmt.Sprintf("%-*s", availableSpace, repsPart)
		formattedTarget := formattedReps + modifiersPart

		fmt.Printf(tableIndent+"│%-*d│%-*s│%-*s│%-*s│\n",
			setColWidth, setIdx+1,
			targetColWidth, formattedTarget,
			currentColWidth, setStr,
			prevColWidth, prevSet,
		)
	}
	fmt.Println(bottomBorder)
	fmt.Println()
}

// New helper: printExerciseDetailsWithIndex prints a single exercise’s details with its index.
func printExerciseDetailsWithIndex(se models.SessionExercise, idx int, tableIndent string, setColWidth, targetColWidth, currentColWidth, prevColWidth int, horizontalBorder, headerLine, midBorder, bottomBorder string, cyan, yellow, red, green func(a ...interface{}) string) {
	ex := se.Exercise
	var technique string
	if se.Technique != "" {
		technique = yellow("(Technique: " + se.Technique + ")")
	}
	// Print the exercise header using the provided index.
	fmt.Printf("%d - %s %s\n", idx, cyan(ex.Name), technique)
	// Optional metadata.
	if !ex.LastPerformed.IsZero() {
		fmt.Printf("   %s %s\n", cyan("Last performed:"), ex.LastPerformed.Format("2006-01-02"))
	}
	if ex.BestSet != nil {
		fmt.Printf("   %s %.1fkg × %d (1RM: %.1fkg)\n",
			cyan("All-time PR:"), ex.BestSet.Weight, ex.BestSet.Reps,
			utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps))
	}
	if se.ProgramNotes != "" {
		fmt.Printf("   %s %s\n", cyan("Program Notes:"), se.ProgramNotes)
	}
	if se.SessionNotes != "" {
		fmt.Printf("   %s %s\n", green("Session Notes:"), se.SessionNotes)
	}
	if len(se.Options) > 0 {
        fmt.Printf("   %s %s\n", cyan("Available options:"), strings.Join(se.Options, ", "))
	}

	// Print the table header for sets.
	fmt.Println(horizontalBorder)
	fmt.Println(headerLine)
	fmt.Println(midBorder)

	// Print each set.
	for setIdx, set := range se.Sets {
		// Build previous set string.
		var prevSet string
		if setIdx < len(se.PreviousSets) {
			ps := se.PreviousSets[setIdx]
			if ps.Weight == 0 && ps.Reps == 0 {
				prevSet = "First time"
			} else {
				prevSet = fmt.Sprintf("%.1fkg × %d", ps.Weight, ps.Reps)
			}
		} else {
			prevSet = "N/A"
		}

		// Build current set string.
		var setStr string
		if set.Bodyweight {
			setStr = fmt.Sprintf("Bodyweight × %d", set.Reps)
		} else if set.Weight == 0 {
			setStr = "Not completed"
		} else {
			setStr = fmt.Sprintf("%.1fkg × %d", set.Weight, set.Reps)

			existing1RM := float32(0)
			if ex.BestSet != nil {
				existing1RM = utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps)
			}
			current1RM := utils.CalculateEpley1RM(set.Weight, set.Reps)
			if current1RM > existing1RM && !strings.EqualFold(se.Technique, models.TechniqueMyoreps) && !strings.EqualFold(se.Technique, models.TechniqueHell) {
				setStr += " ★"
			}
		}

		// Build target string.
		var targetRep string
		if setIdx < len(se.TargetReps) {
			targetRep = se.TargetReps[setIdx]
		}
		// Append target RPE and RM% info.
		var parts []string
		if setIdx < len(se.TargetRPE) {
			parts = append(parts, fmt.Sprintf("@%.1f", se.TargetRPE[setIdx]))
		}
		if setIdx < len(se.TargetRMPercent) && se.Program1RM != nil {
			calculated := *se.Program1RM * (se.TargetRMPercent[setIdx] / 100)
			parts = append(parts, fmt.Sprintf("@%.0f%% (%.1fkg)", se.TargetRMPercent[setIdx], calculated))
		}
		if len(parts) > 0 {
			targetRep += " " + strings.Join(parts, "/")
		}

		splitRep := strings.SplitN(targetRep, " ", 2)
		repsPart := splitRep[0]
		modifiersPart := ""
		if len(splitRep) > 1 {
			modifiersPart = splitRep[1]
		}

		availableSpace := targetColWidth - len(modifiersPart)
		if availableSpace < 0 {
			availableSpace = 0
		}

		formattedReps := fmt.Sprintf("%-*s", availableSpace, repsPart)
		formattedTarget := formattedReps + modifiersPart

		fmt.Printf(tableIndent+"│%-*d│%-*s│%-*s│%-*s│\n",
			setColWidth, setIdx+1,
			targetColWidth, formattedTarget,
			currentColWidth, setStr,
			prevColWidth, prevSet,
		)
	}
	fmt.Println(bottomBorder)
	fmt.Println()
}

func init() {
	rootCmd.AddCommand(showSessionCmd)
}
