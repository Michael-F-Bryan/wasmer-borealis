/*
Copyright © 2023 NAME HERE <EMAIL ADDRESS>
*/
package main

import (
	"go.uber.org/zap"
)

func main() {
	rootCmd := rootCommand()

	err := rootCmd.Execute()
	if err != nil {
		zap.L().Fatal("Command failed", zap.Error(err))
	}
}
