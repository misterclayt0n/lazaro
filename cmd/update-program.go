package cmd

import (
	"fmt"
	"os"

	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var updateProgramCmd = &cobra.Command{
	Use:   "update-program [file]",
	Short: "Update an existing program based on a TOML file without losing session data",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		data, err := os.ReadFile(args[0])
		if err != nil {
			return fmt.Errorf("Failed to read file: %w", err)
		}

		st := storage.NewStorage()
		if err := st.UpdateProgram(data); err != nil {
			return fmt.Errorf("Failed to update program: %w", err)
		}

		fmt.Println("✅ Program updated successfully")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(updateProgramCmd)
}
