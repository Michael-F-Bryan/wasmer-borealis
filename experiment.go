package wasmer_borealis

// Experiment instructs Wasmer Borealis how to run an experiment against the
// Wasmer registry.
type Experiment struct {
	// A package which will run on every Wasmer package in the experiment.
	Package string `json:"package"`
	// Arguments passed through to the package.
	Args []string `json:"args"`
	// The command to run.
	//
	// Primarily used when the package doesn't specify an entrypoint and there
	// are multiple commands available.
	Command string `json:"command,omitempty"`
	// Environment variables that should be set for the package.
	Env     map[string]string `json:"env,omitempty"`
	Filters Filters
}

// Filters are used to determine which packages an experiment will be run
// against.
type Filters struct {
	// Packages that should be ignored.
	Blacklist []string `json:"blacklist,omitempty"`
	// Should every version of the package be published, or just the most recent
	// one?
	IncludeEveryVersion bool `json:"include-every-version,omitempty"`
	// If provided, the experiment will be limited to running packages under
	// just these namespaces.
	Namespaces []string `json:"namespaces,omitempty"`
	// If provided, the experiment will be limited to running packages under
	// just these users.
	Users []string `json:"users,omitempty"`
}

// Wasmer is used to configure the `wasmer` CLI.
type Wasmer struct {
	// Additional arguments to pass to the `wasmer` CLI.
	Args []string `json:"args,omitempty"`
	// Environment variables passed to the `wasmer` CLI.
	Env map[string]string `json:"env,omitempty"`
	// Which `wasmer` CLI should we use?
	//
	// If this is a valid Semver version number, that version will be downloaded
	// from GitHub Releases. Otherwise, it is interpreted as the path to an
	// executable to use.
	//
	// Defaults to the latest released version if not provided.
	Version string `json:"version,omitempty"`
}
