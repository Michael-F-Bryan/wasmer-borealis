package wasmer_borealis

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"runtime"
	"sync"
	"time"

	"github.com/Khan/genqlient/graphql"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

const defaultUserAgent = "wasmer-borealis"

// ProductionEndpoint is the GraphQL endpoint used by Wasmer's production
// registry.
const ProductionEndpoint = "https://registry.wasmer.io/graphql"

// DevelopmentEndpoint is the GraphQL endpoint for Wasmer's staging environment.
const DevelopmentEndpoint = "https://registry.wasmer.wtf/graphql"

var maxConcurrentDownloads = runtime.NumCPU() * 2

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
			err := SynchroniseRegistries(ctx, logger.Named("sync"), db, client)
			logger.Error("Sync failed", zap.Error(err))
		}
	}
}

// SynchroniseRegistries wil fetch all package versions from all known registries.
func SynchroniseRegistries(
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
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	for _, registry := range registries {
		wg.Add(1)
		go func(r Registry) {
			defer wg.Done()
			synchroniseRegistry(ctx, logger.Named(r.Endpoint), db, client, r, errorChan)

		}(registry)
	}

	// Collect all errors in the background
	resultChan := make(chan error)
	go func() {
		var allErrors []error
		for err := range errorChan {
			if err != nil && !errors.Is(err, context.Canceled) {
				allErrors = append(allErrors, err)
				cancel()
			}
		}

		switch len(allErrors) {
		case 0:
			resultChan <- nil
		case 1:
			resultChan <- allErrors[0]
		default:
			panic("TODO: Handle multiple errors")
		}
	}()

	wg.Wait()
	close(errorChan)
	return <-resultChan
}

func synchroniseRegistry(
	ctx context.Context,
	logger *zap.Logger,
	db *gorm.DB,
	client *http.Client,
	registry Registry,
	errorChan chan<- error,
) {
	logger.Info("Syncing registry")
	partials := make(chan partialPackageVersion)
	gql := graphql.NewClient(registry.Endpoint, &authClient{inner: client, token: registry.Token})

	go func() {
		defer close(partials)
		err := fetchAllPackages(ctx, logger, gql, partials)
		if err != nil {
			errorChan <- fmt.Errorf("unable to fetch all packages for %s: %w", registry.Endpoint, err)
		}
	}()

	wg := sync.WaitGroup{}
	downloads := make(chan downloadedPackage)

	for i := 0; i < maxConcurrentDownloads; i++ {
		wg.Add(1)

		go func() {
			defer wg.Done()

			for p := range partials {
				logger.Debug("Downloading package", zap.Any("pkg", p))

				downloaded, err := downloadPackage(
					ctx,
					logger.Named(fmt.Sprintf("%s/%s", p.Owner, p.Package)),
					client,
					p,
				)

				if err == nil {
					downloads <- downloaded
				} else {
					errorChan <- fmt.Errorf("unable to download %s/%s from %s: %w", p.Owner, p.Package, registry.Endpoint, err)
				}
			}
		}()
	}

	go func() {
		for d := range downloads {
			err := savePartialPackage(db, logger, registry, d.partial, d.webc, d.tarball)
			if err != nil {
				errorChan <- fmt.Errorf("unable to save partial package %s: %w", d.partial.fullName(), err)
			}
		}
	}()

	wg.Wait()
	close(downloads)
}

func savePartialPackage(
	db *gorm.DB,
	logger *zap.Logger,
	registry Registry,
	partial partialPackageVersion,
	webc []byte,
	tarball []byte,
) error {
	owner := Owner{
		RegistryID: registry.ID,
		Name:       partial.Owner,
		OwnerType:  partial.OwnerType,
	}
	if err := db.Where(&owner).FirstOrCreate(&owner).Error; err != nil {
		return fmt.Errorf("unable to save the owner: %w", err)
	}

	pkg := Package{
		OwnerID: owner.ID,
		Name:    partial.Owner,
	}
	if err := db.Where(&pkg).FirstOrCreate(&pkg).Error; err != nil {
		return fmt.Errorf("unable to save the package: %w", err)
	}

	pv := PackageVersion{
		PackageID:  pkg.ID,
		UpstreamID: partial.UpstreamID,
		Version:    partial.Version,
	}

	// Make sure we aren't creating duplicates
	var matches int64
	if err := db.Where(&pv).Count(&matches).Error; err != nil && matches > 0 {
		logger.Debug("Already downloaded", zap.Any("package-version", pv))
		return nil
	}

	if webc != nil {
		hash := sha256.Sum256(webc)
		webcBlob := Blob{
			Sha256: hex.EncodeToString(hash[:]),
			Bytes:  webc,
		}
		if err := db.Where(&webcBlob).FirstOrCreate(&webcBlob).Error; err != nil {
			return fmt.Errorf("unable to save the webc: %w", err)
		}
		pv.WebcId = webcBlob.ID
	}
	if tarball != nil {
		hash := sha256.Sum256(tarball)
		tarballBlob := Blob{
			Sha256: hex.EncodeToString(hash[:]),
			Bytes:  tarball,
		}
		if err := db.Where(&tarballBlob).FirstOrCreate(&tarballBlob).Error; err != nil {
			return fmt.Errorf("unable to save the tarball: %w", err)
		}
		pv.TarballId = tarballBlob.ID
	}

	if err := db.Where(&pv).FirstOrCreate(&pv).Error; err != nil {
		return err
	}

	return nil
}

type downloadedPackage struct {
	partial partialPackageVersion
	webc    []byte
	tarball []byte
}

func downloadPackage(ctx context.Context, logger *zap.Logger, client *http.Client, pkg partialPackageVersion) (downloadedPackage, error) {
	downloaded := downloadedPackage{
		partial: pkg,
	}

	if pkg.TarballUrl != "" {
		tarball, err := downloadFile(ctx, client, pkg.TarballUrl)
		if err != nil {
			return downloadedPackage{}, err
		}
		logger.Debug(
			"Downloaded tarball",
			zap.String("url", pkg.TarballUrl),
			zap.Int("bytes-downloaded", len(tarball)),
		)
		downloaded.tarball = tarball
	}

	if pkg.WebcUrl != "" {
		webc, err := downloadFile(ctx, client, pkg.WebcUrl)
		if err != nil {
			return downloadedPackage{}, err
		}
		logger.Debug(
			"Downloaded webc",
			zap.String("url", pkg.WebcUrl),
			zap.Int("bytes-downloaded", len(webc)),
		)
		downloaded.webc = webc
	}

	return downloaded, nil
}

func downloadFile(ctx context.Context, client *http.Client, url string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request to %s failed: %w", url, err)
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("downloading %s failed with %s", url, resp.Status)
	}

	defer resp.Body.Close()

	return io.ReadAll(resp.Body)
}

func fetchAllPackages(ctx context.Context, logger *zap.Logger, client graphql.Client, ch chan<- partialPackageVersion) error {
	after := ""
	for {
		logger.Info("Fetching a page")
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
		Package:    pkg.PackageName,
		Owner:      owner.GetGlobalName(),
		OwnerType:  ownerType,
		Version:    node.Version,
		UpstreamID: node.Id,
		WebcUrl:    node.Distribution.WebcDownloadUrl,
		TarballUrl: node.Distribution.DownloadUrl,
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

func (p partialPackageVersion) fullName() string {
	return fmt.Sprintf("%s/%s@%s", p.Owner, p.Package, p.Version)
}

type authClient struct {
	inner graphql.Doer
	token string
}

func (a *authClient) Do(req *http.Request) (*http.Response, error) {
	if req.Header.Get("Authorization") == "" && a.token != "" {
		req.Header.Set("Authorization", "Bearer "+a.token)
	}

	if req.Header.Get("User-Agent") == "" {
		req.Header.Set("User-Agent", defaultUserAgent)
	}

	return a.inner.Do(req)
}
