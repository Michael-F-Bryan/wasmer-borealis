use std::path::PathBuf;

use anyhow::{Context, Error};
use clap::Parser;
use reqwest::{header::HeaderMap, Client, ClientBuilder, Url};
use wasmer_borealis::{config::Document, experiment::ExperimentBuilder};

#[derive(Parser, Debug)]
pub struct Run {
    /// The Wasmer registry to query packages from.
    #[clap(long, default_value = "wasmer.io", env = "WASMER_REGISTRY")]
    registry: String,
    #[clap(long, short, env = "WASMER_TOKEN")]
    token: Option<String>,
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

        let client = self.client()?;
        let mut builder = ExperimentBuilder::new(experiment)
            .with_endpoint(url)?
            .with_client(client);

        if let Some(output) = self.output {
            builder = builder.with_experiment_dir(output);
        }

        let results = builder.run()?;

        let stdout = std::io::stdout();
        wasmer_borealis::render::text(&results, &mut stdout.lock())?;
        println!("Experiment dir: {}", results.experiment_dir.display());

        Ok(())
    }

    fn client(&self) -> Result<Client, Error> {
        let builder = ClientBuilder::new();
        let mut headers = HeaderMap::new();

        headers.insert(
            reqwest::header::USER_AGENT,
            crate::USER_AGENT.parse().unwrap(),
        );

        if let Some(token) = self.token.as_deref() {
            let auth_header = format!("bearer {token}").parse()?;
            headers.append(reqwest::header::AUTHORIZATION, auth_header);
        }

        let client = builder.default_headers(headers).build()?;

        Ok(client)
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
