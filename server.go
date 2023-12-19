package wasmer_borealis

import (
	"errors"
	"net/http"

	"github.com/gorilla/mux"
	"github.com/graphql-go/graphql"
	"github.com/graphql-go/handler"
	"go.uber.org/zap"
	"gorm.io/gorm"
)

type Server struct {
	db     *gorm.DB
	logger *zap.Logger
	cache  packageCache
}

func NewServer(
	db *gorm.DB,
	logger *zap.Logger,
	cache packageCache,
) *Server {
	return &Server{db, logger, cache}
}

func (s *Server) Router() http.Handler {
	r := mux.NewRouter()

	schema := s.graphqlSchema()

	r.Handle("/graphql", handler.New(&handler.Config{
		Schema:     &schema,
		Pretty:     true,
		GraphiQL:   true,
		Playground: true,
	}))

	return r
}

func (s *Server) graphqlSchema() graphql.Schema {
	experimentType := graphql.NewObject(graphql.ObjectConfig{
		Name:        "Experiment",
		Description: "Information about a running experiment",
		Fields: graphql.Fields{
			"ID": &graphql.Field{
				Type:        graphql.Int,
				Description: "The experiment ID",
			},
			"CreatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this experiment was created",
			},
			"UpdatedAt": &graphql.Field{
				Type:        graphql.DateTime,
				Description: "When this experiment was last updated",
			},
			"Definition": &graphql.Field{
				Type:        graphql.String,
				Description: "The raw JSON definition for this experiment",
			},
		},
	})

	rootQuery := graphql.NewObject(graphql.ObjectConfig{
		Name: "RootQuery",
		Fields: graphql.Fields{
			"getExperiment": &graphql.Field{
				Description: "Get an experiment by ID",
				Type:        experimentType,
				Resolve:     s.resolveGetExperiment,
				Args: graphql.FieldConfigArgument{
					"id": &graphql.ArgumentConfig{
						Type: graphql.Int,
					},
				},
			},
			"getExperiments": &graphql.Field{
				Description: "List all known experiments",
				Type:        graphql.NewList(experimentType),
				Resolve:     s.resolveGetExperiments,
			},
		},
	})

	schema, err := graphql.NewSchema(graphql.SchemaConfig{
		Query: rootQuery,
	})
	if err != nil {
		s.logger.Panic("The GraphQL schema is invalid")
	}

	return schema
}

func (s *Server) resolveGetExperiments(p graphql.ResolveParams) (interface{}, error) {
	s.logger.Info("Resolving experiments")

	var experiments []RunningExperiment
	if err := s.db.WithContext(p.Context).Find(&experiments).Error; err != nil {
		return nil, err
	}

	return experiments, nil
}

func (s *Server) resolveGetExperiment(p graphql.ResolveParams) (interface{}, error) {
	id, ok := p.Args["id"].(int)
	if !ok {
		return nil, errors.New("missing ID")
	}

	s.logger.Info("Resolving experiment", zap.Int("id", id))

	var exp RunningExperiment
	if err := s.db.WithContext(p.Context).Where("id = ?", id).First(&exp).Error; err != nil {
		return nil, err
	}

	return exp, nil
}
