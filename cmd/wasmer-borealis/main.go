/*
Copyright © 2023 NAME HERE <EMAIL ADDRESS>
*/
package main

import (
	"fmt"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"go.uber.org/zap"
)

var DefaultUserAgent = fmt.Sprintf("wasmer-borealis/%s", wasmer_borealis.Version)

func main() {
	rootCmd := rootCommand()

	err := rootCmd.Execute()
	if err != nil {
		zap.L().Fatal("Command failed", zap.Error(err))
	}
}
