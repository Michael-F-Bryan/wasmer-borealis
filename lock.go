package wasmer_borealis

import (
	"context"
	"errors"
	"sync"

	"go.uber.org/zap"
)

var errResolvePanicked = errors.New("resolve panicked")

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
