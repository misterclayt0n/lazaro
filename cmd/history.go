package cmd

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/spf13/cobra"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
)

var (
	filterProgram string
	filterBlock   string
	filterDay     string
)

// historyCmd shows overall session history grouped by program and day.
var historyCmd = &cobra.Command{
	Use:   "history",
	Short: "Display overall session history, optionally filtered by program and/or block and/or day",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		sessions, err := st.GetAllSessions()
		if err != nil {
			return fmt.Errorf("failed to retrieve sessions: %w", err)
		}

		// Case insensitive filtering by program name.
		if filterProgram != "" {
			var filtered []*models.TrainingSession
			for _, s := range sessions {
				// Get the program name for this session.
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

		// Case insensitive filtering by block name.
		if filterBlock != "" {
			var filtered []*models.TrainingSession
			for _, s := range sessions {
				// Get the block name for this session.
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

		// If filtering by day.
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
				sessionDay := s.StartTime.Format("2006-01-02")
				if sessionDay == parsedDay.Format("2006-01-02") {
					filtered = append(filtered, s)
				}
			}
			sessions = filtered
		}

		// Group sessions by program name (using the new helper) and then by day.
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

		// Sort and display the history.
		var programKeys []string
		for p := range grouped {
			programKeys = append(programKeys, p)
		}
		sort.Strings(programKeys)
		for _, prog := range programKeys {
			fmt.Printf("Program: %s\n", prog)
			var days []string
			for d := range grouped[prog] {
				days = append(days, d)
			}
			sort.Strings(days)
			for _, d := range days {
				fmt.Printf("  Date: %s\n", d)
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
					fmt.Printf("    Session %s | Start: %s | Duration: %s\n",
						s.ID,
						s.StartTime.Format("15:04"),
						duration,
					)
				}
			}
			fmt.Println()
		}

		return nil
	},
}

func init() {
	rootCmd.AddCommand(historyCmd)
	historyCmd.Flags().StringVarP(&filterProgram, "program", "p", "", "Filter by program name (case insensitive)")
	historyCmd.Flags().StringVarP(&filterBlock, "block", "b", "", "Filter by block name (case insensitive)")
	historyCmd.Flags().StringVarP(&filterDay, "day", "d", "", "Filter by day (e.g. 2025-02-07 or 07/02/25)")
}
