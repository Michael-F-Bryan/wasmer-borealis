// A helper command for updating the experiment.schema.json file based on the
// Experiment type's JSON schema.
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"log"
	"os"
	"path/filepath"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
	"github.com/invopop/jsonschema"
)

func main() {
	var output string

	flag.StringVar(&output, "out", "experiment.schema.json", "Where to save the generated JSON schema")
	flag.Parse()

	schema := jsonschema.Reflect(&wasmer_borealis.Experiment{})

	schemaJson, err := schema.MarshalJSON()
	if err != nil {
		log.Fatalf("Unable to serialize the schema: %s", err)
	}

	buffer := bytes.Buffer{}
	err = json.Indent(&buffer, schemaJson, "", "  ")
	if err != nil {
		log.Fatalf("Unable to pretty-print the schema: %s", err)
	}

	parent := filepath.Dir(output)
	err = os.MkdirAll(parent, 0766)
	if err != nil {
		log.Fatalf("Unable to create the %s directory", parent)
	}

	err = os.WriteFile(output, buffer.Bytes(), 0666)
	if err != nil {
		log.Fatalf("Unable to write the JSON schema to %s: %s", output, err)
	}
}
