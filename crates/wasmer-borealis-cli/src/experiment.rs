use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use semver::Version;

/// A Wasmer Borealis experiment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Experiment {
    /// The name of the package used when running the experiment.
    pub package: String,
    /// The command to run.
    ///
    /// Primarily used when the package doesn't specify an entrypoint and there
    /// are multiple commands available.
    pub command: Option<String>,
    /// Arguments that should be passed through to the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<TemplatedString>,
    /// Environment variables that should be set for the package.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: IndexMap<String, TemplatedString>,
    #[serde(default, skip_serializing_if = "should_show_wasmer_config")]
    pub wasmer: WasmerConfig,
}

/// Configuration for the `wasmer` CLI being used.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct WasmerConfig {
    /// Which `wasmer` CLI should we use?
    #[serde(default, skip_serializing_if = "WasmerVersion::is_latest")]
    pub version: WasmerVersion,
    /// Environment variables passed to the `wasmer` CLI.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: IndexMap<String, TemplatedString>,
}

fn should_show_wasmer_config(cfg: &WasmerConfig) -> bool {
    let WasmerConfig { version, env } = cfg;
    version.is_latest() && env.is_empty()
}

/// The `wasmer` CLI version to use.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum WasmerVersion {
    /// A local binary.
    Local {
        /// The path.
        path: PathBuf,
    },
    /// A released version.
    #[cfg_attr(test, schemars(with = "VersionRef"))]
    Release(Version),
    /// Use the most recent version.
    #[default]
    Latest,
}

impl WasmerVersion {
    fn is_latest(&self) -> bool {
        matches!(self, WasmerVersion::Latest)
    }
}

/// A string that supports environment variable interpolation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct TemplatedString(String);

impl TemplatedString {
    pub fn new(s: impl Into<String>) -> Self {
        TemplatedString(s.into())
    }

    pub fn raw(&self) -> &str {
        &self.0
    }

    pub fn resolve(&self, home: &Path, get_env: impl Fn(&str) -> Option<String>) -> Cow<'_, str> {
        shellexpand::full_with_context_no_errors(&self.0, || home.to_str(), |var| get_env(var))
    }
}

impl From<String> for TemplatedString {
    fn from(value: String) -> Self {
        TemplatedString::new(value)
    }
}

/// A semver-compatible version number.
#[cfg(test)]
#[derive(schemars::JsonSchema)]
#[serde(remote = "Version")]
struct VersionRef(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiment_schema_is_up_to_date() {
        let project_root = project_root();
        let dest = project_root.join("experiment.schema.json");
        let schema = schemars::schema_for!(Experiment);
        let schema = serde_json::to_string_pretty(&schema).unwrap();

        ensure_file_contents(dest, schema);
    }

    /// Get the root directory for this repository.
    fn project_root() -> &'static Path {
        let root_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        assert!(root_dir.join(".git").exists());

        root_dir
    }

    /// Check that a particular file has the desired contents.
    ///
    /// If the file is missing or outdated, this function will update the file and
    /// trigger a panic to fail any test this is called from.
    fn ensure_file_contents(path: impl AsRef<Path>, contents: impl AsRef<str>) {
        let path = path.as_ref();
        let contents = normalize_newlines(contents.as_ref());

        if let Ok(old_contents) = std::fs::read_to_string(path) {
            if contents == normalize_newlines(&old_contents) {
                // File is already up to date
                return;
            }
        }

        let display_path = path.strip_prefix(project_root()).unwrap_or(path);

        eprintln!("{} was not up-to-date, updating...", display_path.display());

        if std::env::var("CI").is_ok() {
            eprintln!("Note: run `cargo test` locally and commit the updated files");
        }

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, contents).unwrap();
        panic!("some file was not up to date and has been updated. Please re-run the tests.");
    }

    fn normalize_newlines(s: &str) -> String {
        s.replace("\r\n", "\n")
    }
}
