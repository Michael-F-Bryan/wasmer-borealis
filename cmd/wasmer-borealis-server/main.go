package main

import (
	"flag"

	"go.uber.org/zap"
)

func main() {
	args := parseArgs()
	logger := args.initializeLogger()

	defer logger.Info("Shutting down")
	defer logger.Sync()

	logger.Info("Started!")
}

type args struct {
	port    int
	host    string
	devMode bool
}

func parseArgs() args {
	var args args

	flag.BoolVar(&args.devMode, "dev", false, "Enable developer mode")
	flag.IntVar(&args.port, "port", 8080, "The port to serve on")
	flag.StringVar(&args.host, "host", "localhost", "The interface to serve on")

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
