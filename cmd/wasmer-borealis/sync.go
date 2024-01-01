package main

import (
	"context"
	"net/http"
	"os"
	"os/signal"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
)

func syncCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use: "sync",
		Run: sync,
	}

	registerDbArgs(cmd)

	return cmd
}

func sync(cmd *cobra.Command, args []string) {
	logger := zap.L()

	db, err := initDb()
	if err != nil {
		logger.Fatal("Unable to initialize the database", zap.Error(err))
	}
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()

	go func() {
		<-ctx.Done()
		logger.Error("DONE!!")
	}()

	client := http.Client{}

	if err := wasmer_borealis.SynchroniseRegistries(ctx, logger, db, &client); err != nil {
		logger.Fatal("Sync failed", zap.Error(err))
	}
}
