package wasmer_borealis

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"time"

	"github.com/gorilla/mux"
	"github.com/graphql-go/graphql"
	"github.com/graphql-go/handler"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

func NewServer(
	db *gorm.DB,
	logger *zap.Logger,
) http.Handler {
	r := mux.NewRouter()

	r.Use(
		WrapContextMiddleware(
			SetRequestID(),
			SetLogger(logger),
			SetDatabase(db),
		),
		requestIDMiddleware(),
		serverHeaderMiddleware(),
		loggingMiddleware(),
		mux.CORSMethodMiddleware(r),
	)

	schema, err := graphqlSchema()
	if err != nil {
		logger.Panic("The GraphQL schema is invalid", zap.Error(err))
	}

	r.Handle("/graphql", handler.New(&handler.Config{
		Schema:     &schema,
		Pretty:     true,
		GraphiQL:   true,
		Playground: true,
	})).Methods(http.MethodGet, http.MethodPost, http.MethodOptions, http.MethodHead)

	r.HandleFunc("/healthz", healthcheck).Methods(http.MethodOptions, http.MethodGet, http.MethodHead)

	return r
}

func graphqlSchema() (graphql.Schema, error) {
	dbObject := graphql.NewInterface(graphql.InterfaceConfig{
		Name: "DatabaseObject",
		Fields: graphql.Fields{
			"ID": &graphql.Field{
				Type:        graphql.Int,
				Description: "The object's ID",
			},
			"CreatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was created",
			},
			"UpdatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was last updated",
			},
		},
	})
	testCase := graphql.NewObject(graphql.ObjectConfig{
		Name: "TestCase",
		Fields: graphql.Fields{
			"ID": &graphql.Field{
				Type:        graphql.Int,
				Description: "The object's ID",
			},
			"CreatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was created",
			},
			"UpdatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was last updated",
			},
			"State": &graphql.Field{
				Type: graphql.String,
			},
		},
		Interfaces: []*graphql.Interface{dbObject},
	})
	experimentType := graphql.NewObject(graphql.ObjectConfig{
		Name:        "Experiment",
		Description: "Information about a running experiment",
		Fields: graphql.Fields{
			"ID": &graphql.Field{
				Type:        graphql.Int,
				Description: "The object's ID",
			},
			"CreatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was created",
			},
			"UpdatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this object was last updated",
			},
			"Definition": &graphql.Field{
				Type:        graphql.String,
				Description: "The raw JSON definition for this experiment",
			},
			"TestCases": &graphql.Field{
				Type:    graphql.NewList(testCase),
				Resolve: resolveTestCases,
			},
		},
		Interfaces: []*graphql.Interface{dbObject},
	})

	rootQuery := graphql.NewObject(graphql.ObjectConfig{
		Name: "RootQuery",
		Fields: graphql.Fields{
			"getExperiment": &graphql.Field{
				Description: "Get an experiment by ID",
				Type:        experimentType,
				Resolve:     resolveGetExperiment,
				Args: graphql.FieldConfigArgument{
					"id": &graphql.ArgumentConfig{
						Type: graphql.Int,
					},
				},
			},
			"getExperiments": &graphql.Field{
				Description: "List all known experiments",
				Type:        graphql.NewList(experimentType),
				Resolve:     resolveGetExperiments,
			},
		},
	})

	return graphql.NewSchema(graphql.SchemaConfig{
		Query: rootQuery,
	})
}

func resolveGetExperiments(p graphql.ResolveParams) (interface{}, error) {
	db := GetDatabase(p.Context)
	logger := GetLogger(p.Context)

	logger.Info("Resolving experiments")

	var experiments []Experiment
	if err := db.WithContext(p.Context).Find(&experiments).Error; err != nil {
		return nil, err
	}

	return experiments, nil
}

func resolveGetExperiment(p graphql.ResolveParams) (interface{}, error) {
	db := GetDatabase(p.Context)
	logger := GetLogger(p.Context)

	id, ok := p.Args["id"].(int)
	if !ok {
		return nil, errors.New("missing ID")
	}

	logger.Info("Resolving experiment", zap.Int("id", id))

	var exp Experiment
	if err := db.WithContext(p.Context).Where("id = ?", id).First(&exp).Error; err != nil {
		return nil, err
	}

	return exp, nil
}

func resolveTestCases(p graphql.ResolveParams) (interface{}, error) {
	db := GetDatabase(p.Context)
	exp := p.Info.RootValue.(Experiment)

	filter := TestCase{ExperimentID: exp.ID}
	var testCases []TestCase
	if err := db.Where(&filter).Scan(&testCases).Error; err != nil {
		return nil, err
	}

	return testCases, nil
}

func healthcheck(w http.ResponseWriter, r *http.Request) {

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	var response healthCheckResponse

	db, err := GetDatabase(r.Context()).DB()
	if err != nil {
		response.Database.Error = err
	} else if err = db.PingContext(ctx); err != nil {
		response.Database.Error = err
	} else {
		response.Database.Ok = true
	}

	response.Ok = response.Database.Ok

	w.Header().Add("Content-Type", "application/json")

	if response.Ok {
		w.WriteHeader(http.StatusOK)
	} else {
		w.WriteHeader(http.StatusInternalServerError)
	}

	if err := json.NewEncoder(w).Encode(&response); err != nil {
		GetLogger(r.Context()).Error("Unable to write the healthcheck", zap.Error(err))
	}
}

type healthCheckResponse struct {
	Ok       bool     `json:"ok"`
	Database dbHealth `json:"db"`
}

type dbHealth struct {
	Ok    bool  `json:"ok"`
	Error error `json:"error,omitempty"`
}
