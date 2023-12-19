package main

import (
	"flag"
	"fmt"
	"net/http"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"go.uber.org/zap"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func main() {
	args := parseArgs()
	logger := args.initializeLogger()
	defer func() { _ = logger.Sync() }()
	defer logger.Info("Shutting down")

	addr := args.address()
	logger.Info("Started!", zap.String("url", fmt.Sprintf("http://%s/", addr)))

	db, err := args.db()
	if err != nil {
		logger.Fatal("unable to initialize the database", zap.Error(err))
	}

	server := wasmer_borealis.NewServer(db, logger, nil)

	if err := http.ListenAndServe(addr, server.Router()); err != nil {
		logger.Fatal("Unable to start the server", zap.Error(err))
	}
}

type args struct {
	port     int
	host     string
	devMode  bool
	registry string
}

func parseArgs() args {
	var args args

	flag.BoolVar(&args.devMode, "dev", false, "Enable developer mode")
	flag.IntVar(&args.port, "port", 8080, "The port to serve on")
	flag.StringVar(&args.host, "host", "localhost", "The interface to serve on")
	flag.StringVar(&args.registry, "registry", wasmer_borealis.ProductionEndpoint, "The GraphQL endpoint to use when discovering packages")

	flag.Parse()

	return args
}

func (a args) initializeLogger() *zap.Logger {
	var cfg zap.Config

	if a.devMode {
		cfg = zap.NewDevelopmentConfig()
	} else {
		cfg = zap.NewProductionConfig()
	}

	return zap.Must(cfg.Build())
}

func (a args) address() string {
	return fmt.Sprintf("%s:%d", a.host, a.port)
}

func (a args) db() (*gorm.DB, error) {
	db, err := gorm.Open(sqlite.Open(":memory:"))
	if err != nil {
		return nil, fmt.Errorf("unable to open the database: %w", err)
	}

	err = wasmer_borealis.AutoMigrate(db)
	if err != nil {
		return nil, fmt.Errorf("Unable to apply migrations: %w", err)
	}

	return db, nil
}
