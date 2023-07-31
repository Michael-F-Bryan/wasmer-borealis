use std::path::PathBuf;

use anyhow::{Context, Error};

#[derive(Debug, clap::Parser)]
pub struct Report {
    #[clap(long)]
    html: Option<PathBuf>,
    #[clap(long)]
    open: bool,
    json: PathBuf,
}

impl Report {
    pub fn execute(self) -> Result<(), Error> {
        let raw = std::fs::read_to_string(&self.json)?;
        let results: wasmer_borealis::experiment::Results = serde_json::from_str(&raw)?;

        wasmer_borealis::render::text(&results, std::io::stdout())?;

        if self.open || self.html.is_some() {
            let html = self
                .html
                .or_else(|| Some(self.json.parent()?.join("report.html")))
                .context("Unable to determine the html path")?;

            if let Some(parent) = html.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let rendered = wasmer_borealis::render::html(&results)?;
            std::fs::write(&html, rendered)?;

            if self.open {
                open::that_detached(html)?;
            }
        }

        Ok(())
    }
}
