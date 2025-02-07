package cmd

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var statusCmd = &cobra.Command{
	Use:   "status",
	Short: "Show meta data: total weight lifted, session count, gym hours, week streak, and sets per muscle (current week)",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		// Retrieve all basic sessions.
		basicSessions, err := st.GetAllSessions()
		if err != nil {
			return fmt.Errorf("failed to retrieve sessions: %w", err)
		}

		// Load full session details (with exercises and sets) for each session.
		var sessions []*models.TrainingSession
		for _, s := range basicSessions {
			fullSession, err := st.GetSessionByID(s.ID)
			if err != nil {
				continue // skip sessions that fail to load
			}
			sessions = append(sessions, fullSession)
		}

		var totalWeight float32
		var totalSessions int
		var totalDuration time.Duration
		muscleSetsThisWeek := make(map[string]int)
		now := time.Now()
		currentYear, currentWeek := now.ISOWeek()

		// Aggregate data from each session.
		for _, s := range sessions {
			totalSessions++
			if s.EndTime != nil {
				totalDuration += s.EndTime.Sub(s.StartTime)
			}
			for _, se := range s.Exercises {
				// Sum up the total weight (weight × reps) from every set.
				for _, set := range se.Sets {
					if set.Weight > 0 && set.Reps > 0 {
						totalWeight += set.Weight * float32(set.Reps)
					}
				}
				// If the session is in the current ISO week, tally the number of sets by primary muscle.
				year, week := s.StartTime.ISOWeek()
				if year == currentYear && week == currentWeek {
					muscle := se.Exercise.PrimaryMuscle
					muscleSetsThisWeek[muscle] += len(se.Sets)
				}
			}
		}

		// Compute the week streak.
		weekStreak := computeWeekStreak(sessions)

		// Print a stylish header.
		printBoxedHeader("STATUS")

		// Print metrics using a helper.
		printMetric("Total weight lifted", fmt.Sprintf("%.1f kg", totalWeight))
		printMetric("Total sessions", totalSessions)
		printMetric("Total time at gym", totalDuration.Round(time.Minute))
		printMetric("Week streak", fmt.Sprintf("%d weeks", weekStreak))
		fmt.Println()

		// Print the sets per muscle (for the current week) in a stylish list.
		header := color.New(color.FgGreen, color.Bold).Sprintf("Sets per muscle (current week):")
		fmt.Println(header)
		var muscles []string
		for m := range muscleSetsThisWeek {
			muscles = append(muscles, m)
		}
		sort.Strings(muscles)
		for _, m := range muscles {
			// Each muscle name in bold magenta with a bullet.
			fmt.Printf("  • %s: %d sets\n", color.New(color.FgMagenta, color.Bold).Sprint(m), muscleSetsThisWeek[m])
		}
		fmt.Println()

		return nil
	},
}

// printBoxedHeader prints the title in a Unicode box with a fixed width.
func printBoxedHeader(title string) {
	width := 40
	cyanBold := color.New(color.FgCyan, color.Bold).SprintFunc()
	border := strings.Repeat("═", width)
	fmt.Println(cyanBold("╔" + border + "╗"))
	fmt.Println(cyanBold("║" + centerText2(title, width) + "║"))
	fmt.Println(cyanBold("╚" + border + "╝"))
}

func centerText2(s string, width int) string {
	if len(s) >= width {
		return s
	}
	padding := (width - len(s)) / 2
	return strings.Repeat(" ", padding) + s + strings.Repeat(" ", width-len(s)-padding)
}

// printMetric prints a label and value using bold yellow for the label.
func printMetric(label string, value interface{}) {
	yellowBold := color.New(color.FgYellow, color.Bold).SprintFunc()
	fmt.Printf("  %s: %v\n", yellowBold(label), value)
}

// computeWeekStreak computes how many consecutive ISO weeks (ending with the current week)
// have at least one session.
func computeWeekStreak(sessions []*models.TrainingSession) int {
	weekSet := make(map[string]bool)
	for _, s := range sessions {
		year, week := s.StartTime.ISOWeek()
		key := fmt.Sprintf("%d-%02d", year, week)
		weekSet[key] = true
	}

	streak := 0
	now := time.Now()
	year, week := now.ISOWeek()
	for {
		key := fmt.Sprintf("%d-%02d", year, week)
		if weekSet[key] {
			streak++
		} else {
			break
		}
		now = now.AddDate(0, 0, -7)
		year, week = now.ISOWeek()
	}
	return streak
}

func init() {
	rootCmd.AddCommand(statusCmd)
}
