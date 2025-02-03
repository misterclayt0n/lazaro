package cmd

import (
	"fmt"
	"os"

	"github.com/BurntSushi/toml"
	"github.com/misterclayt0n/lazaro/internal/models"
	"github.com/misterclayt0n/lazaro/internal/storage"
	"github.com/spf13/cobra"
)

var deleteProgramCmd = &cobra.Command{
	Use:   "delete-program [file]",
	Short: "Delete a program specified in a TOML file",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		// Read toml file.
		data, err := os.ReadFile(args[0])
		if err != nil {
			return fmt.Errorf("Failed to read file: %w", err)
		}

		var progTOML models.ProgramTOML
		if err := toml.Unmarshal(data, &progTOML); err != nil {
			return fmt.Errorf("Failed to parse TOML: %w", err)
		}

		if progTOML.Name == "" {
			return fmt.Errorf("Program name not specified in TOML file")
		}

		st := storage.NewStorage()
		if err := st.DeleteProgramByName(progTOML.Name); err != nil {
			return fmt.Errorf("Failed to delete program: %w", err)
		}

		fmt.Printf("✅ Program '%s' deleted successfully\n", progTOML.Name)
		return nil
	},
}

func init() {
	rootCmd.AddCommand(deleteProgramCmd)
}
