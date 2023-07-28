use std::path::PathBuf;

use anyhow::Error;
use clap::Parser;
use futures::StreamExt;
use reqwest::Client;

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

        let url = format!("https://registry.{registry}/graphql");
        let client = Client::new();

        let (sender, mut receiver) = futures::channel::mpsc::channel(16);

        tokio::spawn(async move {
            while let Some(pkg) = receiver.next().await {
                println!("{pkg:#?}");
            }
        });

        crate::queries::all_packages_in_namespace(&client, &url, "wasmer", sender.clone()).await?;

        Ok(())
    }
}
