use std::path::PathBuf;

use anyhow::{Context, Error};
use clap::Parser;
use wasmer_borealis::{config::Document, experiment::ExperimentBuilder};

#[derive(Parser, Debug)]
pub struct Run {
    /// The Wasmer registry to query packages from.
    #[clap(long, default_value = "wasmer.io")]
    registry: String,
    /// A directory all experiment-related files will be written to.
    #[clap(short, long)]
    output: Option<PathBuf>,
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

        let mut builder = ExperimentBuilder::new(experiment).with_endpoint(url);

        if let Some(output) = self.output {
            builder = builder.with_experiment_dir(output);
        }

        let results = builder.run()?;

        let stdout = std::io::stdout();
        wasmer_borealis::render::text(&results, &mut stdout.lock())?;
        println!("Experiment dir: {}", results.experiment_dir.display());

        Ok(())
    }
}
