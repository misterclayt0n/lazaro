package cmd

import (
	"fmt"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var (
	dateStr string
)

var lookSessionCmd = &cobra.Command{
	Use:   "look-session [session-id]",
	Short: "Display detailed information for a training session by its ID, or by date using --date",
	// Allow 0 or 1 argument.
	Args: cobra.MaximumNArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		// Define color functions.
		boldGreen := color.New(color.FgGreen, color.Bold).SprintFunc()
		blue := color.New(color.FgBlue).SprintFunc()
		yellow := color.New(color.FgYellow).SprintFunc()
		magenta := color.New(color.FgMagenta).SprintFunc()
		cyan := color.New(color.FgCyan).SprintFunc()
		red := color.New(color.FgRed).SprintFunc()

		// If the --date flag is provided, search by date.
		if dateStr != "" {
			// Assume the user passes the date as "DD/MM/YY"
			userDate, err := time.Parse("02/01/06", dateStr)
			if err != nil {
				return fmt.Errorf("Failed to parse date, please use DD/MM/YY format: %w", err)
			}
			isoDate := userDate.Format("2006-01-02") // e.g. "2025-02-05"

			// Use our new storage function to get sessions by date.
			sessionsSummary, err := st.GetSessionsByDate(isoDate)
			if err != nil {
				return fmt.Errorf("Failed to retrieve sessions for date %s: %w", isoDate, err)
			}
			if len(sessionsSummary) == 0 {
				fmt.Println(magenta("No sessions found on that date."))
				return nil
			}

			// Print header.
			fmt.Println(boldGreen("Training Sessions on:"), yellow(isoDate))
			fmt.Println(strings.Repeat("=", 50))

			// For each summary session, load full details by session ID.
			for i, sum := range sessionsSummary {
				// Load the full session.
				session, err := st.GetSessionByID(sum.ID)
				if err != nil {
					// Fallback: use the summary if full details fail.
					session = &sum
				}

				// Calculate duration if the session is complete.
				var duration string
				if session.EndTime != nil {
					dur := session.EndTime.Sub(session.StartTime).Round(time.Second)
					duration = dur.String()
				} else {
					duration = "In Progress"
				}

				// Print header information.
				fmt.Printf("\n%s %d. %s\n", boldGreen("Session"), i+1, session.ID)
				fmt.Printf("   %s: %s\n", cyan("Start Time"), session.StartTime.Format(time.RFC1123))
				if session.EndTime != nil {
					fmt.Printf("   %s: %s\n", blue("End Time"), session.EndTime.Format(time.RFC1123))
					fmt.Printf("   %s: %s\n", red("Duration"), duration)
				} else {
					fmt.Printf("   %s: %s\n", red("Duration"), "In Progress")
				}
				if session.Notes != "" {
					fmt.Printf("   %s: %s\n", magenta("Session Notes"), session.Notes)
				}
				fmt.Println(strings.Repeat("─", 50))

				// Print each exercise within the session.
				if len(session.Exercises) == 0 {
					fmt.Println("   " + magenta("No exercises found in this session."))
				} else {
					for j, se := range session.Exercises {
						fmt.Printf("%s %d. %s\n", boldGreen("Exercise"), j+1, se.Exercise.Name)
						fmt.Printf("   %s: %s\n", yellow("Description"), se.Exercise.Description)

						// Print the sets in a table format.
						sets := se.Sets
						if len(sets) > 0 {
							fmt.Println("   " + boldGreen("Sets:"))
							fmt.Printf("      %-4s | %-12s | %-5s\n", "Set", "Weight (kg)", "Reps")
							fmt.Println("      " + strings.Repeat("─", 30))
							for k, set := range sets {
								fmt.Printf("      %-4d | %-12.1f | %-5d\n", k+1, set.Weight, set.Reps)
							}
						} else {
							fmt.Println("   " + magenta("No set data available."))
						}
						fmt.Println()
					}
				}
				fmt.Println()
			}

			return nil
		}

		// Otherwise, search by session ID.
		if len(args) != 1 {
			return fmt.Errorf("Please provide a session ID or use the --date flag")
		}

		sessionID := args[0]
		session, err := st.GetSessionByID(sessionID)
		if err != nil {
			return fmt.Errorf("Failed to retrieve session: %w", err)
		}

		// Calculate duration if complete.
		var duration string
		if session.EndTime != nil {
			dur := session.EndTime.Sub(session.StartTime).Round(time.Second)
			duration = dur.String()
		} else {
			duration = "In Progress"
		}

		// Print session details.
		fmt.Println(boldGreen("Training Session Details:"))
		fmt.Printf("  %s: %s\n", cyan("Session ID"), session.ID)
		fmt.Printf("  %s: %s\n", blue("Start Time"), session.StartTime.Format(time.RFC1123))
		if session.EndTime != nil {
			fmt.Printf("  %s: %s\n", blue("End Time"), session.EndTime.Format(time.RFC1123))
		}
		fmt.Printf("  %s: %s\n", red("Duration"), duration)
		if session.Notes != "" {
			fmt.Printf("  %s: %s\n", magenta("Session Notes"), session.Notes)
		}
		fmt.Println(strings.Repeat("=", 50))
		fmt.Println()

		if len(session.Exercises) == 0 {
			fmt.Println(magenta("No exercises found in this session."))
			return nil
		}
		for i, se := range session.Exercises {
			fmt.Printf("%s %d. %s\n", boldGreen("Exercise"), i+1, se.Exercise.Name)
			fmt.Printf("   %s: %s\n", yellow("Description"), se.Exercise.Description)

			// Print the sets in a table format like a real champ.
			sets := se.Sets
			if len(sets) > 0 {
				fmt.Println("   " + boldGreen("Sets:"))
				fmt.Printf("      %-4s | %-12s | %-5s\n", "Set", "Weight (kg)", "Reps")
				fmt.Println("      " + strings.Repeat("─", 30))
				for j, set := range sets {
					fmt.Printf("      %-4d | %-12.1f | %-5d\n", j+1, set.Weight, set.Reps)
				}
			} else {
				fmt.Println("   " + magenta("No set data available."))
			}
			fmt.Println()
		}

		return nil
	},
}

func init() {
	// Now our command accepts zero or one argument.
	rootCmd.AddCommand(lookSessionCmd)
	lookSessionCmd.Flags().StringVarP(&dateStr, "date", "d", "", "Search for sessions by date (in DD/MM/YY format)")
}
