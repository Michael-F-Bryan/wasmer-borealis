package wasmer_borealis

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"

	"github.com/invopop/jsonschema"
	"github.com/kylelemons/godebug/diff"
	"github.com/stretchr/testify/assert"
)

func TestExperimentSchema_IsUpToDate(t *testing.T) {
	currentSchemaBytes, err := os.ReadFile("experiment.schema.json")
	assert.NoError(t, err)
	currentSchema := string(currentSchemaBytes)

	schema := jsonschema.Reflect(&Experiment{})

	schemaJson, err := schema.MarshalJSON()
	assert.NoError(t, err)
	buffer := bytes.Buffer{}
	err = json.Indent(&buffer, schemaJson, "", "  ")
	assert.NoError(t, err)

	newSchema := buffer.String()

	if currentSchema != newSchema {
		t.Log(diff.Diff(currentSchema, newSchema))
		if _, ci := os.LookupEnv("CI"); ci {
			t.Log("Note: run `go test ./...` locally and commit the updated files")
		}
		os.WriteFile("experiment.schema.json", []byte(newSchema), 0666)
		t.Fatal("Experiment schema has changed. Please re-run the tests.")
	}
}
