package cmd

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

// details is a flag to enable verbose session details.
var details bool

// calendarCmd prints the calendar grid.
// Days with training sessions are printed with a color based on the session’s program (block) name,
// and a legend is printed below the calendar.
// If the --details (or -v) flag is set, additional session information is printed.
var calendarCmd = &cobra.Command{
	Use:   "calendar [month] [year]",
	Short: "Display a calendar of training days with a legend mapping colors to session programs",
	Args:  cobra.RangeArgs(0, 2),
	RunE: func(cmd *cobra.Command, args []string) error {
		// Determine month and year (default to current month/year).
		now := time.Now()
		month := now.Month()
		year := now.Year()
		if len(args) >= 1 {
			m, err := strconv.Atoi(args[0])
			if err != nil || m < 1 || m > 12 {
				return fmt.Errorf("invalid month: %s", args[0])
			}
			month = time.Month(m)
		}
		if len(args) == 2 {
			y, err := strconv.Atoi(args[1])
			if err != nil || y < 1 {
				return fmt.Errorf("invalid year: %s", args[1])
			}
			year = y
		}

		// Compute the first and last day of the month.
		firstOfMonth := time.Date(year, month, 1, 0, 0, 0, 0, time.Local)
		lastOfMonth := firstOfMonth.AddDate(0, 1, -1)

		// Query sessions between first and last day.
		st := storage.NewStorage()
		sessions, err := st.GetSessionsBetween(firstOfMonth, lastOfMonth)
		if err != nil {
			return fmt.Errorf("failed to get sessions: %w", err)
		}

		// Group sessions by day and build a set of program identifiers.
		sessionsByDay := make(map[int][]*models.TrainingSession)
		programSet := make(map[string]bool)
		for _, s := range sessions {
			day := s.StartTime.In(time.Local).Day()
			sessionsByDay[day] = append(sessionsByDay[day], s)

			// Use the program name from the training session’s Program field.
			// (The helper function GetProgramSessionName looks it up via the session’s program_block_id.)
			prog, err := st.GetProgramSessionName(s.ID)
			if err != nil || strings.TrimSpace(prog) == "" {
				prog = "Default"
			}
			programSet[prog] = true
		}

		// Define a fixed palette of colors.
		colorPalette := []color.Attribute{
			color.FgRed, color.FgGreen, color.FgYellow,
			color.FgBlue, color.FgMagenta, color.FgCyan,
		}
		// Create a map from program identifier to a color function.
		programColors := make(map[string]func(a ...interface{}) string)
		i := 0
		for prog := range programSet {
			programColors[prog] = color.New(colorPalette[i%len(colorPalette)]).SprintFunc()
			i++
		}

		// Print the calendar header.
		header := fmt.Sprintf("%s %d", month.String(), year)
		fmt.Println(centerText(header, 20))
		fmt.Println("Su Mo Tu We Th Fr Sa")

		// Determine weekday of first day (0 = Sunday).
		weekday := int(firstOfMonth.Weekday())
		// Print initial empty slots.
		for i := 0; i < weekday; i++ {
			fmt.Print("   ")
		}

		// Print day numbers.
		for day := 1; day <= lastOfMonth.Day(); day++ {
			dayStr := fmt.Sprintf("%2d", day)
			if sessList, hasSession := sessionsByDay[day]; hasSession {
				// Use the program session name from the first session of that day.
				prog, err := st.GetProgramSessionName(sessList[0].ID)
				if err != nil || strings.TrimSpace(prog) == "" {
					prog = "Default"
				}
				if colFunc, ok := programColors[prog]; ok {
					dayStr = colFunc(dayStr + "*")
				} else {
					dayStr = color.New(color.FgWhite).Sprint(dayStr + "*")
				}
			}
			fmt.Printf("%s ", dayStr)
			weekday++
			if weekday%7 == 0 {
				fmt.Println()
			}
		}
		fmt.Println("\n") // Extra newline after the calendar

		// Print a legend mapping colors to program names.
		fmt.Println("Legend:")
		for prog, colFunc := range programColors {
			fmt.Printf("  %s: %s\n", colFunc("██"), prog)
		}

		// If the details flag is set, print additional session details.
		if details {
			fmt.Println("\nSession Details:")
			// Extract and sort the days for which we have sessions.
			var days []int
			for d := range sessionsByDay {
				days = append(days, d)
			}
			sort.Ints(days)
			for _, day := range days {
				dayDate := time.Date(year, month, day, 0, 0, 0, 0, time.Local)
				fmt.Printf("\n%s:\n", dayDate.Format("Mon, 02 Jan 2006"))
				for _, sess := range sessionsByDay[day] {
					prog, err := st.GetProgramSessionName(sess.ID)
					if err != nil || strings.TrimSpace(prog) == "" {
						prog = "Default"
					}
					// Print session ID, program session name, and start/end times.
					fmt.Printf("  Session %s (%s) at %s", sess.ID, prog, sess.StartTime.Format("15:04"))
					if sess.EndTime != nil {
						fmt.Printf(" - %s", sess.EndTime.Format("15:04"))
					}
					fmt.Println()
				}
			}
		}

		return nil
	},
}

// centerText centers the given string in a field of the specified width.
func centerText(s string, width int) string {
	if len(s) >= width {
		return s
	}
	padding := (width - len(s)) / 2
	return strings.Repeat(" ", padding) + s
}

func init() {
	rootCmd.AddCommand(calendarCmd)
	calendarCmd.Flags().BoolVarP(&details, "details", "d", false, "Print additional session details")
}
