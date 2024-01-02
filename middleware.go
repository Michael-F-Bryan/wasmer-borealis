package wasmer_borealis

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/google/uuid"
	"github.com/gorilla/mux"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

// loggingMiddleware automatically logs all requests when they are handled.
func loggingMiddleware() mux.MiddlewareFunc {
	return func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			started := time.Now()
			spyWriter := spyResponseWriter{
				inner: w,
				code:  http.StatusOK,
			}
			logger := GetLogger(r.Context())
			logger.Info("Asdf")

			h.ServeHTTP(&spyWriter, r)

			logger.Info("Handled request",
				zap.String("method", r.Method),
				zap.Stringer("url", r.URL),
				zap.Int("code", spyWriter.code),
				zap.Int("bytes-written", spyWriter.bytesWritten),
				zap.String("referrer", r.Referer()),
				zap.Duration("duration", time.Since(started)),
			)
		})
	}
}

type dbKeyType struct{}
type loggerKeyType struct{}
type requestIDKeyType struct{}

var dbKey = &dbKeyType{}
var loggerKey = &loggerKeyType{}
var requestIDKey = &requestIDKeyType{}

// GetRequestID will get the request ID stored in the context by SetLogger().
func GetRequestID(ctx context.Context) uuid.UUID {
	if requestID, ok := ctx.Value(requestIDKey).(uuid.UUID); ok {
		return requestID
	} else {
		panic(fmt.Sprintf("No request ID attached to the context: %s", ctx))
	}
}

func SetRequestID() WrapContextFunc {
	fmt.Println("returning request ID context func")
	return func(ctx context.Context) context.Context {
		requestID := uuid.New()
		fmt.Println("Request ID", requestID.String())
		return context.WithValue(ctx, requestIDKey, requestID)
	}
}

// SetDatabase is a wrapContextFunc which will attach a database handle to the
// context so it can be retrieved later using GetDatabase().
func SetDatabase(db *gorm.DB) WrapContextFunc {
	return func(ctx context.Context) context.Context {
		return context.WithValue(ctx, dbKey, db)
	}
}

// GetDatabase will get the database handle stored in this context by
// SetDatabase.
func GetDatabase(ctx context.Context) *gorm.DB {
	if db, ok := ctx.Value(dbKey).(*gorm.DB); ok {
		return db
	} else {
		panic("No database attached to the context")
	}
}

// SetLogger returns a wrapContextFunc that attaches a logger to a context so it
// can be retrieved later using getLogger().
func SetLogger(logger *zap.Logger) WrapContextFunc {
	return func(ctx context.Context) context.Context {
		requestID := GetRequestID(ctx)
		logger := logger.With(zap.Stringer("request-id", requestID))
		return context.WithValue(ctx, loggerKey, logger)
	}
}

// GetLogger will retrieve the logger stored in the provided context by
// SetLogger.
//
// If no logger was stored, this returns the global zap.L() instance.
func GetLogger(ctx context.Context) *zap.Logger {
	if logger, ok := ctx.Value(loggerKey).(*zap.Logger); ok {
		return logger
	} else {
		return zap.L()
	}
}

type spyResponseWriter struct {
	inner        http.ResponseWriter
	code         int
	bytesWritten int
}

func (s *spyResponseWriter) Write(data []byte) (int, error) {
	bytesWritten, err := s.inner.Write(data)
	s.bytesWritten += bytesWritten
	return bytesWritten, err
}

func (s *spyResponseWriter) Header() http.Header {
	return s.inner.Header()
}

func (s *spyResponseWriter) WriteHeader(code int) {
	s.code = code
	s.inner.WriteHeader(code)
}

// WrapContextMiddleware is a HTTP middleware which update the request's
// context.Context based on the various wrapContextFunc functions passed in.
func WrapContextMiddleware(wrap ...WrapContextFunc) mux.MiddlewareFunc {
	return func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			ctx := WrapContext(r.Context())
			h.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

type WrapContextFunc = func(context.Context) context.Context

// WrapContext applies successive WrapContextFunc functions to a context.Context,
// returning the final value.
//
// This is often used to simplify how values are attached to the context.
func WrapContext(ctx context.Context, wrap ...WrapContextFunc) context.Context {
	for _, w := range wrap {
		ctx = w(ctx)
	}
	return ctx
}

// requestIDMiddleware attaches the request ID to the response as the
// "x-request-id" header.
func requestIDMiddleware() mux.MiddlewareFunc {
	return func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			requestID := GetRequestID(r.Context())
			w.Header().Add("x-request-id", requestID.String())

			h.ServeHTTP(w, r)
		})
	}
}

func serverHeaderMiddleware() mux.MiddlewareFunc {
	server := fmt.Sprintf("wasmer-borealis/%s", Version)

	return func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {

			w.Header().Add("server", server)

			h.ServeHTTP(w, r)
		})
	}
}
