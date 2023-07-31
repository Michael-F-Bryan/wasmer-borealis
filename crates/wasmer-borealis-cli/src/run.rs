use std::path::PathBuf;

use anyhow::{Context, Error};
use clap::Parser;
use wasmer_borealis::{config::Document, experiment::ExperimentBuilder};

#[derive(Parser, Debug)]
pub struct Run {
    #[clap(long, default_value = "wasmer.io")]
    registry: String,
    #[clap(long)]
    cache: Option<PathBuf>,
    /// The experiment to run.
    experiment: PathBuf,
}

impl Run {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn execute(self) -> Result<(), Error> {
        let experiment = std::fs::read_to_string(&self.experiment)
            .with_context(|| format!("Unable to read \"{}\"", self.experiment.display()))?;
        let Document { experiment, .. } = serde_json::from_str(&experiment)
            .context("Unable to deserialize the experiment file")?;

        let url = format!("https://registry.{}/graphql", self.registry);

        let results = ExperimentBuilder::new(experiment)
            .with_endpoint(url)
            .run()?;

        let stdout = std::io::stdout();
        wasmer_borealis::render::text(&results, &mut stdout.lock())?;

        Ok(())
    }
}
