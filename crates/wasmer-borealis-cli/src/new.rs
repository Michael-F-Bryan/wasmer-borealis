use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Error};
use clap::Parser;

use wasmer_borealis::experiment::{Experiment, Filters, TemplatedString, WasmerConfig};

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
            filters: Filters::default(),
        };

        let doc = Document {
            experiment,
            schema: schema_url(),
        };

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

fn schema_url() -> String {
    // FIXME: Remove the `token=` when the repo goes public
    // let repo = env!("CARGO_PKG_REPOSITORY");
    // format!("{repo}/tree/main/experiment.schema.json")
    // "https://raw.githubusercontent.com/Michael-F-Bryan/wasmer-borealis/main/experiment.schema.json?token=GHSAT0AAAAAAB7G4BD7S73QZZTJ3SHVCAUCZGDJOBA".to_string()
    "https://github.com/Michael-F-Bryan/wasmer-borealis/tree/main/experiment.schema.json?token=GHSAT0AAAAAAB7G4BD7S73QZZTJ3SHVCAUCZGDJOBA".to_string()
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
