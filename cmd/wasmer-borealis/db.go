package main

import (
	"fmt"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/adrg/xdg"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

var (
	dbDriver         = "sqlite"
	connectionString = ""
)

func initDb() (*gorm.DB, error) {
	var db *gorm.DB

	switch dbDriver {
	case "sqlite":
		if connectionString == "" {
			path, err := xdg.DataFile("db.sqlite3")
			if err != nil {
				return nil, err
			}
			connectionString = path
		}
		d, err := gorm.Open(sqlite.Open(connectionString))
		if err != nil {
			return nil, fmt.Errorf("unable to open %q: %w", connectionString, err)
		}
		zap.L().Debug("Opened database", zap.String("path", connectionString))
		db = d

	default:
		return nil, fmt.Errorf("unsupported database driver: %s", dbDriver)
	}

	if err := wasmer_borealis.AutoMigrate(db); err != nil {
		return nil, fmt.Errorf("unable to apply migrations: %w", err)
	}

	return db, nil
}

func registerDbArgs(cmd *cobra.Command) {
	cmd.Flags().StringVar(&dbDriver, "db-driver", dbDriver, "The database type")
	cmd.Flags().StringVar(&connectionString, "db", connectionString, "The database to connect to")
}
