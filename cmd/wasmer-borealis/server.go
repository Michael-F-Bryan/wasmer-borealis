package main

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"time"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
)

var (
	port uint16 = 8000
	host        = "localhost"
)

func serverCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "server",
		Short: "Run the wasmer-borealis server",
		Run:   serve,
	}

	cmd.Flags().Uint16VarP(&port, "port", "p", port, "The port to listen on")
	cmd.Flags().StringVar(&host, "host", host, "The interface to listen on")

	registerDbArgs(cmd)

	return cmd
}

func serve(cmd *cobra.Command, args []string) {
	logger := zap.L()
	defer logger.Info("Shutting down")

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()

	db, err := initDb()
	if err != nil {
		logger.Fatal("Unable to initialize the database", zap.Error(err))
	}

	addr := fmt.Sprintf("%s:%d", host, port)
	logger.Info("Serving", zap.String("addr", addr))

	mux := http.Server{
		Addr:    addr,
		Handler: wasmer_borealis.NewServer(db, logger),
	}

	go func() {
		<-ctx.Done()

		logger.Info("Beginning graceful shutdown")

		shutdownContext, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()

		if err := mux.Shutdown(shutdownContext); err != nil {
			logger.Error("Graceful shutdown failed", zap.Error(err))
		}
	}()

	if err := mux.ListenAndServe(); err != http.ErrServerClosed {
		logger.Error("Unable to start the server", zap.Error(err))
		_ = logger.Sync()
		os.Exit(1)
	}
}
