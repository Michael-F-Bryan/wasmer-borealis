package main

import (
	"encoding/json"
	"fmt"
	"os"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

func registryCommand() *cobra.Command {
	var endpoint string
	var token string
	format := formatText

	cmd := &cobra.Command{
		Use:   "registry",
		Short: "Manipulate registries in the database",
	}

	add := &cobra.Command{
		Use:   "add",
		Short: "Add a repository to the database",
		Run: func(cmd *cobra.Command, args []string) {
			repositoryAdd(endpoint, token)
		},
	}
	add.Flags().StringVarP(&endpoint, "endpoint", "e", "", "The URL for the registry's GraphQL endpoint")
	add.MarkFlagRequired("endpoint")
	add.Flags().StringVarP(&token, "token", "t", "", "The API token to use when querying this registry")
	registerDbArgs(add)
	cmd.AddCommand(add)

	list := &cobra.Command{
		Use:   "list",
		Short: "List all known registries",
		Run: func(cmd *cobra.Command, args []string) {
			repositoryList(format)
		},
	}
	list.Flags().VarP(&format, "format", "f", `The output format ("text" or "json")`)
	registerDbArgs(list)
	cmd.AddCommand(list)

	return cmd
}

func repositoryAdd(endpoint string, token string) {
	logger := zap.L()

	db, err := initDb()
	if err != nil {
		logger.Fatal("Unable to initialize the database", zap.Error(err))
	}

	registry := wasmer_borealis.Registry{
		Endpoint: endpoint,
		Token:    token,
	}
	if err := db.Save(&registry).Error; err != nil {
		logger.Fatal(
			"Unable to save the registry",
			zap.Error(err),
			zap.Any("registry", registry),
		)
	}

	logger.Info("Added", zap.Uint("id", registry.ID))
}

func repositoryList(format format) {
	logger := zap.L()

	db, err := initDb()
	if err != nil {
		logger.Fatal("Unable to initialize the database", zap.Error(err))
	}

	var registries []wasmer_borealis.Registry
	err = db.Find(&registries).Error
	if err != nil {
		logger.Fatal("Unable to read the registries", zap.Error(err))
	}

	var results []registryInfo

	for _, r := range registries {
		info, err := loadRegistryInfo(db, r)
		if err != nil {
			logger.Fatal(
				"Unable to load registry info",
				zap.String("registry", r.Endpoint),
				zap.Error(err),
			)
		}
		results = append(results, info)
	}

	switch format {
	case formatText:
		for _, r := range results {
			fmt.Printf("[%d] %s (owners: %d, packages: %d)\n", r.ID, r.Endpoint, r.OwnerCount, r.PackageCount)
		}

	case formatJSON:
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		if err := enc.Encode(results); err != nil {
			logger.Fatal("Unable to write output", zap.Error(err))
		}

	default:
		logger.Fatal("Unknown output format", zap.Stringer("format", format))
	}
}

func loadRegistryInfo(db *gorm.DB, r wasmer_borealis.Registry) (registryInfo, error) {
	var ownerCount int64

	err := db.Where(&wasmer_borealis.Owner{RegistryID: r.ID}).
		Model(&wasmer_borealis.Owner{}).
		Count(&ownerCount).Error
	if err != nil {
		return registryInfo{}, fmt.Errorf("unable to retrieve owners associated with registry %d: %w", r.ID, err)
	}

	var packageCount int64

	err = db.
		Joins("JOIN owners ON packages.owner_id = owners.id AND owners.registry_id = ?", r.ID).
		Model(&wasmer_borealis.Package{}).
		Count(&packageCount).
		Error
	if err != nil {
		return registryInfo{}, fmt.Errorf("unable to retrieve the number of packages associated with registry %d: %w", r.ID, err)
	}

	info := registryInfo{
		ID:           r.ID,
		Endpoint:     r.Endpoint,
		Token:        r.Token,
		OwnerCount:   int(ownerCount),
		PackageCount: int(packageCount),
	}
	return info, nil
}

type format string

const (
	formatText format = "text"
	formatJSON format = "json"
)

var knownFormats = []format{formatText, formatJSON}

func (f format) String() string {
	return string(f)
}

func (f *format) Set(value string) error {
	for _, fmt := range knownFormats {
		if value == string(fmt) {
			*f = fmt
			return nil
		}
	}

	return unknownFormatError{}
}

func (f *format) Type() string {
	return "string"
}

type unknownFormatError struct{}

func (u unknownFormatError) Error() string {
	msg := "expected one of "

	for i, fmt := range knownFormats {
		if i > 0 {
			msg += ", "
		}

		msg += string(fmt)
	}

	return msg
}

type registryInfo struct {
	ID           uint   `json:"id"`
	Endpoint     string `json:"endpoint"`
	Token        string `json:"token"`
	OwnerCount   int    `json:"owner-count"`
	PackageCount int    `json:"package-count"`
}
