package wasmer_borealis

import (
	"context"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/Khan/genqlient/graphql"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

// StartSynchronisingRegistries periodically synchronises the packages.
func StartSynchronisingRegistries(
	ctx context.Context,
	logger *zap.Logger,
	db *gorm.DB,
	client *http.Client,
	interval time.Duration,
) {
loop:
	for {
		select {
		case <-ctx.Done():
			break loop
		case <-time.After(interval):
			synchroniseRegistries(ctx, logger.Named("sync"), db, client)
		}
	}
}

// synchroniseRegistries wil fetch all package versions from all known registries.
func synchroniseRegistries(
	ctx context.Context,
	logger *zap.Logger,
	db *gorm.DB,
	client *http.Client,
) error {
	logger.Info("Started synchronising packages")
	started := time.Now()
	defer func() {
		logger.Info("Finished synchronising", zap.Duration("duration", time.Since(started)))
	}()

	var registries []Registry
	if err := db.Find(&registries).Error; err != nil {
		return err
	}

	errorChan := make(chan error)
	wg := &sync.WaitGroup{}

	for _, registry := range registries {
		wg.Add(1)
		go func(r Registry) {
			defer wg.Done()
			logger := logger.Named(r.Endpoint)
			synchroniseRegistry(ctx, logger.Named(r.Endpoint), db, client, r, errorChan)

		}(registry)
	}

	wg.Done()

	var allErrors []error
	close(errorChan)
	for err := range errorChan {
		if err != nil {
			allErrors = append(allErrors, err)
		}
	}

	switch len(allErrors) {
	case 0:
		return nil
	case 1:
		return allErrors[0]
	default:
		panic("TODO: Handle multiple errors")
	}
}

func synchroniseRegistry(
	ctx context.Context,
	logger *zap.Logger,
	db *gorm.DB,
	client *http.Client,
	registry Registry,
	errorChan chan<- error,
) {
	partials := make(chan partialPackageVersion)
	gql := graphql.NewClient(registry.Endpoint, &authClient{inner: client, token: registry.Token})

	go func() {
		defer close(partials)
		err := fetchAllPackages(ctx, logger, gql, partials)
		if err != nil {
			errorChan <- fmt.Errorf("unable to fetch all packages for %s: %w", registry.Endpoint, err)
		}
	}()

	for p := range partials {
		logger.Debug("Found package", zap.Any("pkg", p))
	}
}

func fetchAllPackages(ctx context.Context, logger *zap.Logger, client graphql.Client, ch chan<- partialPackageVersion) error {
	after := ""
	for {
		resp, err := getAllPackages(ctx, client, after)
		if err != nil {
			return err
		}
		logger.Debug("Retrieved a page of packages", zap.Any("page", resp))

		for _, edge := range resp.AllPackageVersions.Edges {
			pv := edge.Node.partialPackageVersion()

			select {
			case <-ctx.Done():
				return ctx.Err()
			case ch <- pv:
				continue
			}
		}

		after = resp.AllPackageVersions.PageInfo.EndCursor

		if after == "" {
			break
		}
	}

	return nil
}

func (node getAllPackagesAllPackageVersionsPackageVersionConnectionEdgesPackageVersionEdgeNodePackageVersion) partialPackageVersion() partialPackageVersion {
	pkg := node.Package
	owner := pkg.Owner

	var ownerType OwnerType
	switch owner.(type) {
	case *getAllPackagesAllPackageVersionsPackageVersionConnectionEdgesPackageVersionEdgeNodePackageVersionPackageOwnerNamespace:
		ownerType = OwnerNamespace
	case *getAllPackagesAllPackageVersionsPackageVersionConnectionEdgesPackageVersionEdgeNodePackageVersionPackageOwnerPackage:
		ownerType = OwnerUser
	default:
		ownerType = ownerUnknown
	}

	return partialPackageVersion{
		Package:    pkg.Name,
		Owner:      owner.GetGlobalName(),
		OwnerType:  ownerType,
		Version:    node.Version,
		UpstreamID: node.Id,
	}
}

type partialPackageVersion struct {
	Package    string
	Owner      string
	OwnerType  OwnerType
	Version    string
	UpstreamID string
	WebcUrl    string
	TarballUrl string
}
