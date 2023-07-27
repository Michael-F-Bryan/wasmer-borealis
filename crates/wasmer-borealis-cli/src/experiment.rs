use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use semver::Version;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Experiment {
    pub package: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<TemplatedString>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: IndexMap<String, TemplatedString>,
    #[serde(default, skip_serializing_if = "should_show_wasmer_config")]
    pub wasmer: WasmerConfig,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WasmerConfig {
    #[serde(default, skip_serializing_if = "WasmerVersion::is_latest")]
    pub version: WasmerVersion,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: IndexMap<String, TemplatedString>,
}

fn should_show_wasmer_config(cfg: &WasmerConfig) -> bool {
    let WasmerConfig { version, env } = cfg;
    version.is_latest() && env.is_empty()
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum WasmerVersion {
    Local {
        path: PathBuf,
    },
    Release(Version),
    #[default]
    Latest,
}

impl WasmerVersion {
    fn is_latest(&self) -> bool {
        matches!(self, WasmerVersion::Latest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
