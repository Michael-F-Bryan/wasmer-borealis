package wasmer_borealis

import (
	"context"
	"errors"
	"testing"

	"github.com/graphql-go/graphql"
	"github.com/stretchr/testify/assert"
	"go.uber.org/zap/zaptest"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestGraphQLServer_GetExperiments(t *testing.T) {
	server := NewServer(testDb(t), zaptest.NewLogger(t), dummyCache{})
	experiments := []RunningExperiment{
		{Definition: "first"},
		{Definition: "second"},
	}
	assert.NoError(t, server.db.Save(&experiments[0]).Error)
	assert.NoError(t, server.db.Save(&experiments[1]).Error)

	result, err := server.resolveGetExperiments(graphql.ResolveParams{})

	assert.NoError(t, err)
	resolvedExperiment := result.([]RunningExperiment)
	assert.Equal(t, len(experiments), len(resolvedExperiment))
	assert.Equal(t, experiments[0].ID, resolvedExperiment[0].ID)
	assert.Equal(t, experiments[0].Definition, resolvedExperiment[0].Definition)
	assert.Equal(t, experiments[1].ID, resolvedExperiment[1].ID)
	assert.Equal(t, experiments[1].Definition, resolvedExperiment[1].Definition)
}

func TestGraphQLServer_GetExperiment(t *testing.T) {
	server := NewServer(testDb(t), zaptest.NewLogger(t), dummyCache{})
	exp := RunningExperiment{Definition: "asdf"}
	assert.NoError(t, server.db.Save(&exp).Error)

	result, err := server.resolveGetExperiment(graphql.ResolveParams{
		Args: map[string]any{
			"id": int(exp.ID),
		},
	})

	assert.NoError(t, err)
	resolvedExperiment := result.(RunningExperiment)
	*exp.CreatedAt.Location() = *resolvedExperiment.CreatedAt.Location()
	*exp.UpdatedAt.Location() = *resolvedExperiment.UpdatedAt.Location()
	assert.Equal(t, exp, resolvedExperiment)
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

type dummyCache struct{}

func (d dummyCache) lookup(ctx context.Context, pkg packageName, version string) (cachedPackage, error) {
	return cachedPackage{}, errors.New("Unimplemented")
}
