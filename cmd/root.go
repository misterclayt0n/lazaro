package cmd

import (
    "github.com/spf13/cobra"
)

var rootCmd = &cobra.Command {
	Use: "lazaro",
	Short: "CLI training app inspired by boostcamp",
}

func Execute() error {
	return rootCmd.Execute()
}
