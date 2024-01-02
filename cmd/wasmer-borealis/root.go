/*
Copyright © 2023 NAME HERE <EMAIL ADDRESS>
*/
package main

import (
	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

var (
	devMode = false
)

// rootCommand gets the root command that is used when no sub-commands are
// called.
func rootCommand() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:     "wasmer-borealis",
		Short:   "The CLI for interacting with wasmer-borealis",
		Version: wasmer_borealis.Version,
		PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
			var cfg zap.Config

			if devMode {
				cfg = zap.NewDevelopmentConfig()
				cfg.EncoderConfig.EncodeLevel = zapcore.CapitalColorLevelEncoder
			} else {
				cfg = zap.NewProductionConfig()
			}

			logger, err := cfg.Build()
			if err != nil {
				return err
			}

			zap.ReplaceGlobals(logger)
			return nil
		},
		PersistentPostRun: func(cmd *cobra.Command, args []string) {
			_ = zap.L().Sync()
		},
	}

	rootCmd.PersistentFlags().BoolVarP(&devMode, "dev", "v", false, "Enable dev mode")

	rootCmd.AddCommand(serverCommand())
	rootCmd.AddCommand(syncCommand())
	rootCmd.AddCommand(registryCommand())

	return rootCmd
}
