package wasmer_borealis

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync"

	"github.com/Khan/genqlient/graphql"
	"go.uber.org/zap"
)

const defaultUserAgent = "wasmer-borealis"

// ProductionEndpoint is the GraphQL endpoint used by Wasmer's production
// registry.
const ProductionEndpoint = "https://registry.wasmer.io/graphql"

// DevelopmentEndpoint is the GraphQL endpoint for Wasmer's staging environment.
const DevelopmentEndpoint = "https://registry.wasmer.wtf/graphql"

var errResolvePanicked = errors.New("resolve panicked")
var errUnknownPackage = errors.New("no such package")

type packageCache interface {
	// Package will look up a package by name, returning the paths to the cached
	// artifacts.
	//
	// If the version is empty, this will fetch the latest version.
	lookup(ctx context.Context, pkg packageName, version string) (cachedPackage, error)
}

// cachedPackage contains the file paths for packages that have been cached
// locally.
type cachedPackage struct {
	// Tarball contains the path to the package's *.tar.gz file on disk.
	Tarball string
	// Webc contains the path to the *.webc file on disk. If empty, it means the
	// backend doesn't have a *.webc file for the package.
	Webc string
}

// diskCache implements package caching on disk.
type diskCache struct {
	dir    string
	client graphql.Doer
	logger *zap.Logger

	mu         sync.Mutex
	registries map[string]*registrySpecificDiskCache
}

func newDiskCache(cacheDir string, client graphql.Doer, logger *zap.Logger) *diskCache {
	return &diskCache{
		dir:        cacheDir,
		client:     client,
		logger:     logger,
		registries: make(map[string]*registrySpecificDiskCache),
	}
}

// Registry gets a PackageCache implementation specific to a particular registry.
//
// If the token is empty,
func (c *diskCache) Registry(graphqlEndpoint *url.URL, token string) packageCache {
	c.mu.Lock()
	defer c.mu.Unlock()

	host := graphqlEndpoint.Hostname()

	if cache, exists := c.registries[host]; exists {
		return cache
	}

	client := &authClient{c.client, token}
	gql := graphql.NewClient(graphqlEndpoint.String(), client)

	cache := registrySpecificDiskCache{
		dir:        path.Join(c.dir, graphqlEndpoint.Hostname()),
		gql:        gql,
		client:     client,
		logger:     c.logger.With(zap.Stringer("registry", graphqlEndpoint)),
		downloaded: newConcurrentTaskLock[packageVersion, cachedPackage](),
	}
	c.registries[host] = &cache

	return &cache
}

type packageName struct {
	Namespace string
	Name      string
}

func (p packageName) String() string {
	return fmt.Sprintf("%s/%s", p.Namespace, p.Name)
}

type packageVersion struct {
	packageName
	Version string
}

func (pv packageVersion) String() string {
	return fmt.Sprintf("%s/%s@%s", pv.Namespace, pv.Name, pv.Version)
}

type registrySpecificDiskCache struct {
	// The directory all registry-specific files should be saved to.
	dir        string
	logger     *zap.Logger
	gql        graphql.Client
	client     graphql.Doer
	downloaded *concurrentTaskLock[packageVersion, cachedPackage]
}

func (r *registrySpecificDiskCache) lookup(ctx context.Context, pkg packageName, version string) (cachedPackage, error) {
	pv := packageVersion{pkg, version}

	return r.downloaded.Lookup(ctx, pv, func(packageVersion) (cachedPackage, error) {
		dist, err := r.lookupDistribution(ctx, pkg, version)
		if err != nil {
			return cachedPackage{}, err
		}

		return r.downloadDistribution(ctx, pv, dist)
	})
}

func (r *registrySpecificDiskCache) lookupDistribution(ctx context.Context, pkg packageName, version string) (distribution, error) {
	logger := r.logger.With(zap.Stringer("package", pkg))
	var dist distribution

	if version == "" {
		logger.Info("Looking up latest version")

		resp, err := getLatestVersion(ctx, r.gql, pkg.String())
		if err != nil {
			return nil, err
		}
		logger.Debug("received response", zap.Any("response", resp))

		if resp.GetPackage.LastVersion.Id == "" {
			return nil, errUnknownPackage
		}

		version = resp.GetPackage.LastVersion.Version
		logger.Info("Found latest version", zap.String("version", version))

		dist = &resp.GetPackage.LastVersion.Distribution
	} else {
		logger.Info("Looking up fixed version", zap.String("version", version))
		resp, err := getVersion(ctx, r.gql, pkg.String(), version)
		if err != nil {
			return nil, err
		}
		logger.Debug("received response", zap.Any("response", resp))

		if resp.GetPackageVersion.Id == "" {
			return nil, errUnknownPackage
		}

		dist = &resp.GetPackageVersion.Distribution
	}

	if dist.GetDownloadUrl() == "" && dist.GetPiritaDownloadUrl() == "" {
		return nil, errUnknownPackage
	}

	return dist, nil
}

func (r *registrySpecificDiskCache) downloadDistribution(ctx context.Context, pv packageVersion, dist distribution) (cachedPackage, error) {
	logger := r.logger.With(zap.Any("package-version", pv))
	path := r.path(pv)

	var cached cachedPackage

	if tarballUrl := dist.GetDownloadUrl(); tarballUrl != "" {
		path, err := download(ctx, logger, r.client, path, tarballUrl)
		if err != nil {
			return cachedPackage{}, fmt.Errorf("unable to download the tarball from \"%s\": %w", tarballUrl, err)
		}
		cached.Tarball = path
	}

	if webcUrl := dist.GetPiritaDownloadUrl(); webcUrl != "" {
		path, err := download(ctx, logger, r.client, path, webcUrl)
		if err != nil {
			return cachedPackage{}, fmt.Errorf("unable to download the webc from \"%s\": %w", webcUrl, err)
		}
		cached.Webc = path
	}

	logger.Info("downloaded", zap.Any("files", cached))

	return cached, nil
}

func download(ctx context.Context, logger *zap.Logger, client graphql.Doer, outputDir string, rawUrl string) (string, error) {
	logger = logger.With(zap.String("url", rawUrl))

	url, err := url.Parse(rawUrl)
	if err != nil {
		return "", fmt.Errorf("\"%s\" is an invalid URL: %w", rawUrl, err)
	}

	if err := os.MkdirAll(outputDir, 0766); err != nil {
		return "", fmt.Errorf("unable to create the \"%s\" directory: %w", outputDir, err)
	}

	temp, err := os.CreateTemp(outputDir, ".tmp")
	if err != nil {
		return "", fmt.Errorf("unable to create a temporary file in \"%s\": %w", outputDir, err)
	}
	logger.Debug("created temp", zap.String("path", temp.Name()))

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, rawUrl, nil)
	if err != nil {
		return "", err
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	logger.Debug(
		"received response",
		zap.String("status", resp.Status),
		zap.Any("headers", resp.Header),
		zap.Int64("content-length", resp.ContentLength),
	)

	if resp.StatusCode < 200 || 300 < resp.StatusCode {
		return "", fmt.Errorf("request failed with %s", resp.Status)
	}

	if _, err = io.Copy(temp, resp.Body); err != nil {
		return "", fmt.Errorf("unable to read the response: %w", err)
	}

	if err = temp.Sync(); err != nil {
		return "", fmt.Errorf("flushing \"%s\" failed: %w", temp.Name(), err)
	}

	pathSegments := strings.Split(url.Path, "/")
	filename := filepath.Join(outputDir, pathSegments[len(pathSegments)-1])

	if err = os.Rename(temp.Name(), filename); err != nil {
		return "", fmt.Errorf("unable to rename \"%s\" to \"%s\": %w", temp.Name(), filename, err)
	}

	return filename, nil
}

type distribution interface {
	GetDownloadUrl() string
	GetPiritaDownloadUrl() string
}

func (r *registrySpecificDiskCache) path(pkg packageVersion) string {
	return path.Join(r.dir, pkg.Namespace, pkg.Name, pkg.Version)
}

type authClient struct {
	inner graphql.Doer
	token string
}

func (a *authClient) Do(req *http.Request) (*http.Response, error) {
	if req.Header.Get("Authorization") == "" && a.token != "" {
		req.Header.Set("Authorization", a.token)
	}

	if req.Header.Get("User-Agent") == "" {
		req.Header.Set("User-Agent", defaultUserAgent)
	}

	return a.inner.Do(req)
}

// concurrentTaskLock is a cache which
type concurrentTaskLock[Key comparable, Value any] struct {
	mu     sync.Mutex
	values map[Key]*lockResult[Value]
}

func newConcurrentTaskLock[Key comparable, Value any]() *concurrentTaskLock[Key, Value] {
	return &concurrentTaskLock[Key, Value]{
		values: make(map[Key]*lockResult[Value]),
	}
}

func (d *concurrentTaskLock[Key, Value]) Lookup(ctx context.Context, key Key, resolve func(Key) (Value, error)) (Value, error) {
	result := d.entry(key, resolve)

	select {
	case <-result.done:
		return result.value, result.err
	case <-ctx.Done():
		var value Value
		return value, ctx.Err()
	}
}

func (d *concurrentTaskLock[Key, Value]) entry(key Key, resolve func(Key) (Value, error)) *lockResult[Value] {
	d.mu.Lock()
	defer d.mu.Unlock()

	if result, ok := d.values[key]; ok {
		return result
	}

	done := make(chan struct{})

	result := &lockResult[Value]{done: done}
	d.values[key] = result

	go func() {
		defer close(done)
		defer func() {
			if r := recover(); r != nil {
				zap.L().DPanic(
					"panicked in the concurrent task lock",
					zap.Any("key", key),
					zap.Any("recover", r),
				)
				result.err = errResolvePanicked
			}
		}()

		value, err := resolve(key)
		result.value = value
		result.err = err
	}()

	return result
}

type lockResult[T any] struct {
	done  <-chan struct{}
	value T
	err   error
}
