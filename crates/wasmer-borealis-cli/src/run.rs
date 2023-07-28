use std::path::PathBuf;

use anyhow::{Context, Error};
use clap::Parser;
use futures::{channel::mpsc::Receiver, StreamExt};
use reqwest::Client;

use wasmer_borealis::{
    experiment::{Experiment, Filters},
    registry::queries::Package,
};

#[derive(Parser, Debug)]
pub struct Run {
    #[clap(long, default_value = "wasmer.io")]
    registry: String,
    /// The experiment to run.
    experiment: PathBuf,
}

impl Run {
    #[tracing::instrument(level = "debug", skip_all)]
    #[tokio::main]
    pub async fn execute(self) -> Result<(), Error> {
        let Run {
            registry,
            experiment,
        } = self;

        let experiment = std::fs::read_to_string(&experiment)
            .with_context(|| format!("Unable to read \"{}\"", experiment.display()))?;
        let experiment: Experiment = serde_json::from_str(&experiment)
            .context("Unable to deserialize the experiment file")?;

        let url = format!("https://registry.{registry}/graphql");
        let client = Client::new();

        let mut receiver = fetch_packages(client, experiment.filters, url);

        while let Some(pkg) = receiver.next().await {
            println!("{pkg:#?}");
        }

        Ok(())
    }
}

fn fetch_packages(client: Client, filters: Filters, endpoint: String) -> Receiver<Package> {
    let (mut sender, receiver) = futures::channel::mpsc::channel(16);

    tokio::spawn(async move {
        for namespace in &filters.namespaces {
            if let Err(e) = wasmer_borealis::registry::all_packages_in_namespace(
                &client,
                &endpoint,
                "wasmer",
                &mut sender,
            )
            .await
            {
                tracing::error!(
                    error = &*e,
                    namespace = namespace.as_str(),
                    "Unable to fetch a namespace's packages"
                );
            }
        }
    });

    receiver
}
