package wasmer_borealis

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/graphql-go/graphql"
	"github.com/stretchr/testify/assert"
	"go.uber.org/zap"
	"go.uber.org/zap/zaptest"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func testFixtures(t *testing.T) (*gorm.DB, *zap.Logger, context.Context) {
	db := testDb(t)
	logger := zaptest.NewLogger(t)
	ctx := WrapContext(
		context.Background(),
		SetRequestID(),
		SetDatabase(db),
		SetLogger(logger),
	)

	return db, logger, ctx
}

func TestGraphQLServer_GetExperiments(t *testing.T) {
	db, _, ctx := testFixtures(t)
	experiments := []Experiment{
		{Definition: "first"},
		{Definition: "second"},
	}
	assert.NoError(t, db.Save(&experiments[0]).Error)
	assert.NoError(t, db.Save(&experiments[1]).Error)

	result, err := resolveGetExperiments(graphql.ResolveParams{
		Context: ctx,
	})

	assert.NoError(t, err)
	resolvedExperiment := result.([]Experiment)
	assert.Equal(t, len(experiments), len(resolvedExperiment))
	assert.Equal(t, experiments[0].ID, resolvedExperiment[0].ID)
	assert.Equal(t, experiments[0].Definition, resolvedExperiment[0].Definition)
	assert.Equal(t, experiments[1].ID, resolvedExperiment[1].ID)
	assert.Equal(t, experiments[1].Definition, resolvedExperiment[1].Definition)
}

func TestGraphQLServer_GetExperiment(t *testing.T) {
	db, _, ctx := testFixtures(t)
	exp := Experiment{Definition: "asdf"}
	assert.NoError(t, db.Save(&exp).Error)

	result, err := resolveGetExperiment(graphql.ResolveParams{
		Args: map[string]any{
			"id": int(exp.ID),
		},
		Context: ctx,
	})

	assert.NoError(t, err)
	resolvedExperiment := result.(Experiment)
	*exp.CreatedAt.Location() = *resolvedExperiment.CreatedAt.Location()
	*exp.UpdatedAt.Location() = *resolvedExperiment.UpdatedAt.Location()
	assert.Equal(t, exp, resolvedExperiment)
}

func TestHealthcheck(t *testing.T) {
	db, logger, _ := testFixtures(t)
	server := NewServer(db, logger)
	w := httptest.NewRecorder()
	r := httptest.NewRequest(http.MethodGet, "/healthz", nil)

	server.ServeHTTP(w, r)

	assert.Equal(t, http.StatusOK, w.Code)
	var deserializedResponse healthCheckResponse
	assert.NoError(t, json.Unmarshal(w.Body.Bytes(), &deserializedResponse))
	assert.Equal(
		t,
		healthCheckResponse{
			Ok: true,
			Database: dbHealth{
				Ok: true,
			},
		},
		deserializedResponse,
	)
}

func testDb(t *testing.T) *gorm.DB {
	db, err := gorm.Open(sqlite.Open(":memory:"))
	if err != nil {
		t.Fatalf("unable to open the database: %s", err)
	}

	err = AutoMigrate(db)
	if err != nil {
		t.Fatalf("Unable to apply migrations: %s", err)
	}

	return db
}
