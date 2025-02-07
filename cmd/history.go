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

var (
	filterProgram string
	filterBlock   string
	filterDay     string
)

var historyCmd = &cobra.Command{
	Use:   "history",
	Short: "Display overall session history, optionally filtered by program and/or block and/or day",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		// Retrieve all basic sessions.
		sessions, err := st.GetAllSessions()
		if err != nil {
			return fmt.Errorf("failed to retrieve sessions: %w", err)
		}

		// Apply case-insensitive filters if provided.
		if filterProgram != "" {
			var filtered []*models.TrainingSession
			for _, s := range sessions {
				progName, err := st.GetProgramNameForSession(s.ID)
				if err != nil {
					progName = "Unknown"
				}
				if strings.EqualFold(progName, filterProgram) {
					filtered = append(filtered, s)
				}
			}
			sessions = filtered
		}

		if filterBlock != "" {
			var filtered []*models.TrainingSession
			for _, s := range sessions {
				blockName, err := st.GetProgramSessionName(s.ID)
				if err != nil {
					blockName = "Unknown"
				}
				if strings.EqualFold(blockName, filterBlock) {
					filtered = append(filtered, s)
				}
			}
			sessions = filtered
		}

		if filterDay != "" {
			var parsedDay time.Time
			parsedDay, err = time.Parse("2006-01-02", filterDay)
			if err != nil {
				parsedDay, err = time.Parse("02/01/06", filterDay)
			}
			if err != nil {
				return fmt.Errorf("failed to parse day: %w", err)
			}
			var filtered []*models.TrainingSession
			for _, s := range sessions {
				if s.StartTime.Format("2006-01-02") == parsedDay.Format("2006-01-02") {
					filtered = append(filtered, s)
				}
			}
			sessions = filtered
		}

		// Group sessions by program name and then by day.
		grouped := make(map[string]map[string][]*models.TrainingSession)
		for _, s := range sessions {
			progName, err := st.GetProgramNameForSession(s.ID)
			if err != nil {
				progName = "Unknown"
			}
			if _, ok := grouped[progName]; !ok {
				grouped[progName] = make(map[string][]*models.TrainingSession)
			}
			day := s.StartTime.Format("2006-01-02")
			grouped[progName][day] = append(grouped[progName][day], s)
		}

		// Sort program keys.
		var programKeys []string
		for p := range grouped {
			programKeys = append(programKeys, p)
		}
		sort.Strings(programKeys)

		// For each program, print a fancy header, then for each date, print the session lines.
		for _, prog := range programKeys {
			cyanBold := color.New(color.FgCyan, color.Bold).SprintFunc()
			fmt.Printf("%s\n", cyanBold(prog))
			var days []string
			for d := range grouped[prog] {
				days = append(days, d)
			}
			sort.Strings(days)
			for _, d := range days {
				printDateHeader(d)
				sList := grouped[prog][d]
				sort.Slice(sList, func(i, j int) bool {
					return sList[i].StartTime.Before(sList[j].StartTime)
				})
				for _, s := range sList {
					duration := "In progress"
					if s.EndTime != nil {
						dur := s.EndTime.Sub(s.StartTime).Round(time.Second)
						duration = dur.String()
					}
					printSessionLine(s.ID, s.StartTime.Format("15:04"), duration)
				}
				fmt.Println()
			}
			fmt.Println()
		}

		return nil
	},
}

func printDateHeader(date string) {
	// Underline the date header.
	magentaBold := color.New(color.FgMagenta, color.Bold).SprintFunc()
	fmt.Println(magentaBold("  Date: " + date))
}

func printSessionLine(id, start, duration string) {
	// Print a bullet line with session details.
	green := color.New(color.FgGreen).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()
	magenta := color.New(color.FgMagenta).SprintFunc()
	fmt.Printf("    • %s | Start: %s | Duration: %s\n", magenta(id), yellow(start), green(duration))
}

func init() {
	rootCmd.AddCommand(historyCmd)
	historyCmd.Flags().StringVarP(&filterProgram, "program", "p", "", "Filter by program name (case insensitive)")
	historyCmd.Flags().StringVarP(&filterBlock, "block", "b", "", "Filter by block name (case insensitive)")
	historyCmd.Flags().StringVarP(&filterDay, "day", "d", "", "Filter by day (e.g. 2025-02-07 or 07/02/25)")
}
