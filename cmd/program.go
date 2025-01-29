package cmd

import (
	"fmt"
	"os"

	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var createProgramCmd = &cobra.Command{
	Use:   "create-program [file]",
	Short: "Create a new program from TOML file",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()

		file, err := os.ReadFile(args[0])
		if err != nil {
			return fmt.Errorf("failed to read file: %w", err)
		}

		if err := st.CreateProgram(file); err != nil {
			return fmt.Errorf("failed to create program: %w", err)
		}

		fmt.Println("✅ Program created successfully")
		return nil
	},
}

var listProgramsCmd = &cobra.Command{
	Use:   "list-programs",
	Short: "List all programs",
	RunE: func(cmd *cobra.Command, args []string) error {
		st := storage.NewStorage()
		programs, err := st.ListPrograms()
		if err != nil {
			return err
		}

		for _, p := range programs {
			fmt.Printf("%s - %s\n", p.ID, p.Name)
		}
		return nil
	},
}

func init() {
	rootCmd.AddCommand(createProgramCmd)
	rootCmd.AddCommand(listProgramsCmd)
}
