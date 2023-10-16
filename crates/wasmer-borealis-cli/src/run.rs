use std::path::PathBuf;

use anyhow::{Context, Error};
use clap::Parser;
use reqwest::Url;
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

        let url = format_graphql(&self.registry);

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

pub fn format_graphql(registry: &str) -> String {
    if let Ok(mut url) = Url::parse(registry) {
        // Looks like we've got a valid URL. Let's try to use it as-is.
        if url.has_host() {
            if url.path() == "/" {
                // make sure we convert http://registry.wasmer.io/ to
                // http://registry.wasmer.io/graphql
                url.set_path("/graphql");
            }

            return url.to_string();
        }
    }

    if !registry.contains("://") && !registry.contains('/') {
        return endpoint_from_domain_name(registry);
    }

    // looks like we've received something we can't deal with. Just pass it
    // through as-is and hopefully it'll either work or the end user can figure
    // it out
    registry.to_string()
}

/// By convention, something like `"wasmer.io"` should be converted to
/// `"https://registry.wasmer.io/graphql"`.
fn endpoint_from_domain_name(domain_name: &str) -> String {
    if domain_name.contains("localhost") {
        return format!("http://{domain_name}/graphql");
    }

    format!("https://registry.{domain_name}/graphql")
}
