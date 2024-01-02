// Download the latest GraphQL schema from the Wasmer registry.
package main

import (
	"flag"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"

	wasmer_borealis "github.com/Michael-F-Bryan/wasmer-borealis"
)

func main() {
	var registry string
	var output string
	flag.StringVar(&registry, "registry", wasmer_borealis.ProductionEndpoint+"/schema.graphql", "The URL for fetching the schema")
	flag.StringVar(&output, "out", "wasmer-registry.graphql", "Where to save the schema to")

	resp, err := http.Get(registry)
	if err != nil {
		log.Fatalf("Unable to fetch %s: %s", registry, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		log.Fatalf("%s responded with %s", registry, resp.Status)
	}

	schema, err := io.ReadAll(resp.Body)
	if err != nil {
		log.Fatalf("Reading failed: %s", err)
	}

	parent := filepath.Dir(output)
	err = os.MkdirAll(parent, 0766)
	if err != nil {
		log.Fatalf("Unable to create the %s/ directory", parent)
	}

	err = os.WriteFile(output, schema, 0666)
	if err != nil {
		log.Fatalf("Unable to write the GraphQL schema to %s: %s", output, err)
	}
}
