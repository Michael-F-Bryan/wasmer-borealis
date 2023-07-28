use std::path::PathBuf;

use anyhow::{Context, Error};
use once_cell::sync::Lazy;

#[derive(Debug, Clone, clap::Parser)]
pub struct Schema {
    #[clap(short, long, env, default_value = "wasmer.io")]
    registry: String,
    #[clap(short, long, default_value = SCHEMA_PATH.as_os_str())]
    output: PathBuf,
}

impl Schema {
    pub fn run(self) -> Result<(), Error> {
        let Schema { registry, output } = self;

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let url = format!("https://registry.{registry}/graphql/schema.graphql");
        tracing::info!(%url, "Downloading the schema");

        let schema = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async { reqwest::get(&url).await?.error_for_status()?.bytes().await })
            .with_context(|| format!("Unable to fetch the schema from \"{url}\""))?;

        tracing::info!(
            path=%output.display(),
            bytes=schema.len(),
            "Saving to disk",
        );

        std::fs::write(&output, &schema)
            .with_context(|| format!("Unable to save the schema to \"{}\"", output.display()))?;

        Ok(())
    }
}

static SCHEMA_PATH: Lazy<PathBuf> = Lazy::new(|| {
    crate::project_root()
        .join("crates")
        .join("wasmer-borealis-cli")
        .join("src")
        .join("queries")
        .join("schema.graphql")
});
