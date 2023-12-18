package wasmer_borealis

import (
	"context"
	"net/http"
	"net/url"
	"testing"

	"github.com/stretchr/testify/assert"
	"go.uber.org/zap/zaptest"
)

func TestDistributedLock_FirstLookup(t *testing.T) {
	lock := newConcurrentTaskLock[int, int]()

	value, err := lock.Lookup(context.Background(), 1, func(k int) (int, error) { return k, nil })

	assert.NoError(t, err)
	assert.Equal(t, value, 1)
}

func TestDistributedLock_CachedLookup(t *testing.T) {
	lock := newConcurrentTaskLock[int, int]()
	resolve := func(k int) (int, error) { return k, nil }
	_, _ = lock.Lookup(context.Background(), 1, resolve)

	value, err := lock.Lookup(context.Background(), 1, func(k int) (int, error) { panic("resolve is never called") })

	assert.NoError(t, err)
	assert.Equal(t, value, 1)
}

func TestDistributedLock_CatchPanics(t *testing.T) {
	lock := newConcurrentTaskLock[int, int]()

	_, err := lock.Lookup(context.Background(), 0, func(key int) (int, error) {
		panic("deliberately panic")
	})

	assert.ErrorIs(t, err, errResolvePanicked)
}

func TestDistributedLock_LookupTimesOut(t *testing.T) {
	lock := newConcurrentTaskLock[int, int]()
	ctx, cancel := context.WithCancel(context.Background())

	resolve := make(chan struct{})
	firstBeganResolving := make(chan struct{})
	t.Cleanup(func() { close(resolve) })
	second := make(chan error)

	// First, start a lookup which takes a "long time"
	go func() {
		_, _ = lock.Lookup(ctx, 1, func(k int) (int, error) {
			firstBeganResolving <- struct{}{}
			<-resolve
			return k + 1, nil
		})
	}()
	// Wait until the first goroutine begins resolving before starting a second
	// lookup
	<-firstBeganResolving
	go func() {
		_, err := lock.Lookup(ctx, 1, func(k int) (int, error) {
			panic("this resolve is never called")
		})
		second <- err
	}()

	// Cancel the context, which should signal to the second goroutine to
	// error out
	cancel()

	// And the second goroutine should have received an error
	assert.ErrorIs(t, <-second, context.Canceled)
}

func TestDistributedLock_ConcurrentLookups(t *testing.T) {
	lock := newConcurrentTaskLock[int, int]()

	resolve := make(chan struct{})
	firstBeganResolving := make(chan struct{})
	t.Cleanup(func() { close(resolve) })
	second := make(chan int, 2)

	// First, start a lookup which takes a "long time"
	go func() {
		value, _ := lock.Lookup(context.Background(), 1, func(k int) (int, error) {
			firstBeganResolving <- struct{}{}
			<-resolve
			return k + 1, nil
		})
		second <- value
	}()
	// Wait until the first goroutine begins resolving before starting a second
	// lookup
	<-firstBeganResolving
	go func() {
		value, err := lock.Lookup(context.Background(), 1, func(k int) (int, error) {
			panic("this resolve is never called")
		})
		assert.NoError(t, err)
		second <- value
	}()

	// Now that both Lookup calls are pending, let's let the first goroutine
	// resolve
	resolve <- struct{}{}

	// And the second goroutine should have received the same value
	assert.Equal(t, <-second, 2)
	assert.Equal(t, <-second, 2)
}

func TestDiskCache_DownloadCowsay(t *testing.T) {
	if testing.Short() {
		t.SkipNow()
	}

	logger := zaptest.NewLogger(t)
	cache := newDiskCache(t.TempDir(), http.DefaultClient, logger)
	entrypoint, _ := url.Parse(ProductionEndpoint)
	registry := cache.Registry(entrypoint, "")

	cached, err := registry.lookup(context.Background(), packageName{Namespace: "syrusakbary", Name: "cowsay"}, "0.3.0")

	assert.NoError(t, err)
	assert.FileExists(t, cached.Webc)
	assert.FileExists(t, cached.Tarball)
}

func TestDiskCache_DownloadLatestCowsay(t *testing.T) {
	if testing.Short() {
		t.SkipNow()
	}

	logger := zaptest.NewLogger(t)
	cache := newDiskCache(t.TempDir(), http.DefaultClient, logger)
	entrypoint, _ := url.Parse(ProductionEndpoint)
	registry := cache.Registry(entrypoint, "")

	cached, err := registry.lookup(context.Background(), packageName{Namespace: "syrusakbary", Name: "cowsay"}, "")

	assert.NoError(t, err)
	assert.FileExists(t, cached.Webc)
	assert.FileExists(t, cached.Tarball)
}

func TestDiskCache_DownloadNonexistentPackage(t *testing.T) {
	if testing.Short() {
		t.SkipNow()
	}

	logger := zaptest.NewLogger(t)
	cache := newDiskCache(t.TempDir(), http.DefaultClient, logger)
	entrypoint, _ := url.Parse(ProductionEndpoint)
	registry := cache.Registry(entrypoint, "")

	cached, err := registry.lookup(context.Background(), packageName{Namespace: "wasmer", Name: "this-does-not-exist"}, "")

	assert.ErrorIs(t, err, errUnknownPackage)
	assert.Zero(t, cached)
}
