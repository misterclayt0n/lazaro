package cmd

import (
	"fmt"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var (
	programName2 string // Name of the program to show.
	dayFilter    string // Optional day (block name) filter.
)

var showProgramCmd = &cobra.Command{
	Use:   "show-program",
	Short: "Display a visualization of an entire program (optionally filter by day)",
	RunE: func(cmd *cobra.Command, args []string) error {
		if programName2 == "" {
			return fmt.Errorf("program name must be provided via --program flag")
		}

		st := storage.NewStorage()
		prog, err := st.GetProgramByName(programName2)
		if err != nil {
			return fmt.Errorf("failed to load program: %w", err)
		}

		// Set up color functions.
		green := color.New(color.FgGreen).SprintFunc()
		cyan := color.New(color.FgCyan).SprintFunc()
		yellow := color.New(color.FgYellow).SprintFunc()

		// Print program header.
		fmt.Printf("\n%s\n", green(strings.ToUpper(prog.Name)))
		fmt.Printf("%s: %s\n", cyan("Description"), prog.Description)
		fmt.Printf("%s: %s\n", cyan("Created At"), prog.CreatedAt.Format(time.RFC1123))
		fmt.Println(strings.Repeat("=", 60))

		// Iterate through blocks.
		// If a day filter is provided, only show the matching block.
		for _, block := range prog.Blocks {
			if dayFilter != "" && !strings.EqualFold(block.Name, dayFilter) {
				continue
			}

			// Print block header.
			if block.Week != 0 {
				fmt.Printf("\n%s: %s (Week %d)\n", yellow("Day"), block.Name, block.Week)
			} else {
				fmt.Printf("\n%s: %s\n", yellow("Day"), block.Name)
			}
			fmt.Printf("%s: %s\n", yellow("Notes"), block.Description)
			fmt.Println(strings.Repeat("-", 60))

			// Print a list of exercises in this block.
			for i, pe := range block.Exercises {
				// Retrieve the full exercise record to get its name.
				ex, err := st.GetExerciseByID(pe.ExerciseID)
				if err != nil {
					// Fallback to using the ID if lookup fails.
					ex = &models.Exercise{Name: pe.ExerciseID}
				}

				fmt.Printf("%d. %s\n", i+1, ex.Name)

				// Print additional options (variations) if available.
				if len(pe.Options) > 0 {
					fmt.Printf("   %s: %s\n", cyan("Options"), strings.Join(pe.Options, ", "))
				}

				// Build a target scheme display.
				if len(pe.Reps) > 0 {
					var targetParts []string

					// Iterate over all sets (assuming the number of sets is the length of pe.Reps)
					for i, rep := range pe.Reps {
						part := rep
						if i < len(pe.TargetRPE) {
							part += fmt.Sprintf(" (@%.1f RPE)", pe.TargetRPE[i])
						}
						if i < len(pe.TargetRMPercent) {
							part += fmt.Sprintf(" (@%.0f%%)", pe.TargetRMPercent[i])
						}
						targetParts = append(targetParts, part)
					}

					target := strings.Join(targetParts, ", ")
					fmt.Printf("   %s: %s\n", cyan("Target"), target)
				}
				if pe.ProgramNotes != "" {
					fmt.Printf("   %s: %s\n", cyan("Notes"), pe.ProgramNotes)
				}
			}
			fmt.Println()
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(showProgramCmd)
	showProgramCmd.Flags().StringVarP(&programName2, "program", "p", "", "Name of the program (required)")
	showProgramCmd.Flags().StringVarP(&dayFilter, "day", "d", "", "Filter by day (block name)")
	showProgramCmd.MarkFlagRequired("program")
}
