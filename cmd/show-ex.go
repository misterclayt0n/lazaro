package cmd

import (
	"fmt"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/misterclayt0n/lazaro/internal/utils"
	"github.com/spf13/cobra"
)

var (
	limitSessions int
	historyOnly   bool
)

var showExCmd = &cobra.Command{
	Use:   "show-ex [exercise-name]",
	Short: "Display detailed information and training history for a particular exercise",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		exName := args[0]

		st := storage.NewStorage()
		ex, err := st.GetExerciseByName(exName)
		if err != nil {
			return fmt.Errorf("failed to get exercise: %w", err)
		}

		// Define color functions.
		boldGreen := color.New(color.FgGreen, color.Bold).SprintFunc()
		boldCyan := color.New(color.FgCyan, color.Bold).SprintFunc()
		magenta := color.New(color.FgMagenta).SprintFunc()
		yellow := color.New(color.FgYellow).SprintFunc()
		blue := color.New(color.FgBlue).SprintFunc()
		red := color.New(color.FgRed).SprintFunc()

		// If not history-only, print detailed exercise info.
		if !historyOnly {
			fmt.Println(boldGreen("Exercise Information:"))
			fmt.Printf("  %s: %s\n", boldCyan("Name"), ex.Name)
			fmt.Printf("  %s: %s\n", boldCyan("Description"), ex.Description)
			fmt.Printf("  %s: %s\n", boldCyan("Primary Muscle"), ex.PrimaryMuscle)
			fmt.Printf("  %s: %s\n", boldCyan("Created At"), ex.CreatedAt.Format(time.RFC1123))
			if ex.BestSet != nil {
				fmt.Printf("  %s: %.1fkg × %d (%s: %.1fkg)\n",
					boldCyan("All-time PR"),
					ex.BestSet.Weight, ex.BestSet.Reps,
					yellow("Calculated 1RM"), utils.CalculateEpley1RM(ex.BestSet.Weight, ex.BestSet.Reps))
			} else {
				fmt.Printf("  %s: %.1fkg\n", boldCyan("Estimated 1RM"), ex.EstimatedOneRM)
			}
			fmt.Println()
		}

		// Retrieve the training sessions that included this exercise.
		sessions, err := st.GetTrainingSessionsForExercise(ex.ID, limitSessions)
		if err != nil {
			return fmt.Errorf("failed to retrieve session history: %w", err)
		}

		fmt.Printf("%s %s:\n", boldGreen("History for"), ex.Name)
		if len(sessions) == 0 {
			fmt.Println(magenta("  No training sessions found."))
			return nil
		}

		// For each session, load and print the sets.
		for i, ts := range sessions {
			fmt.Printf("\n%s %d. %s\n", boldGreen("Session"), i+1, ts.ID)
			fmt.Printf("   %s: %s\n", blue("Start Time"), utils.FormatSaoPaulo(ts.StartTime))
			if ts.EndTime != nil {
				fmt.Printf("   %s: %s\n", blue("End Time"), utils.FormatSaoPaulo(*ts.EndTime))
				duration := ts.EndTime.Sub(ts.StartTime).Round(time.Second)
				fmt.Printf("   %s: %s\n", red("Duration"), duration)
			}
			if ts.Notes != "" {
				fmt.Printf("   %s: %s\n", magenta("Session Notes"), ts.Notes)
			}

			// Load sets for this exercise in the session.
			sets, err := st.GetExerciseSetsForSession(ts.ID, ex.ID)
			if err != nil {
				return fmt.Errorf("Failed to retrieve sets: %w", err)
			}
			if len(sets) > 0 {
				// Print table header.
				fmt.Println("   " + boldCyan("Sets:"))
				fmt.Printf("      %-4s | %-12s | %-5s\n", "Set", "Weight (kg)", "Reps")
				fmt.Println("      " + strings.Repeat("─", 30))
				for j, set := range sets {
					fmt.Printf("      %-4d | %-12.1f | %-5d\n", j+1, set.Weight, set.Reps)
				}
			} else {
				fmt.Println("   " + magenta("No set data found for this session."))
			}
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(showExCmd)
	showExCmd.Flags().IntVarP(&limitSessions, "limit", "l", 5, "Number of sessions to display")
	showExCmd.Flags().BoolVarP(&historyOnly, "history-only", "H", false, "Display only history (sets and weight) without exercise details")
}
