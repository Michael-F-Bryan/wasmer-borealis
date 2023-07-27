use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Error};
use clap::Parser;

use crate::experiment::{Experiment, TemplatedString, WasmerConfig};

#[derive(Parser, Debug)]
pub struct New {
    /// Where to save the experiment file.
    #[clap(short, long)]
    output: Option<PathBuf>,
    /// Extra environment variables to set for the spawned program.
    #[clap(short, long)]
    env: Vec<EnvironmentVariable>,
    /// The package to test.
    package: String,
    #[clap(last = true)]
    args: Vec<TemplatedString>,
}

impl New {
    pub fn execute(self) -> Result<(), Error> {
        let New {
            output,
            package,
            env,
            args,
        } = self;

        let experiment = Experiment {
            package,
            args,
            command: None,
            env: env
                .into_iter()
                .map(|EnvironmentVariable { name, value }| (name, value))
                .collect(),
            wasmer: WasmerConfig::default(),
        };

        let repo = env!("CARGO_PKG_REPOSITORY");
        let schema = format!("{repo}/tree/main/experiment.schema.json");
        let doc = Document { experiment, schema };

        let yaml = serde_json::to_string_pretty(&doc).context("Serialization failed")?;

        match output {
            Some(path) => {
                std::fs::write(&path, &yaml)
                    .with_context(|| format!("Unable to save to \"{}\"", path.display()))?;
            }
            None => {
                println!("{yaml}");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EnvironmentVariable {
    name: String,
    value: TemplatedString,
}

impl FromStr for EnvironmentVariable {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, value) = s
            .split_once('=')
            .context("Environment variables should be in the form \"key=value\"")?;

        Ok(EnvironmentVariable {
            name: name.to_string(),
            value: TemplatedString::new(value),
        })
    }
}

#[derive(Debug, serde::Serialize)]
struct Document {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(flatten)]
    experiment: Experiment,
}
