use anyhow::Error;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Run {}

impl Run {
    pub fn execute(self) -> Result<(), Error> {
        todo!();
    }
}
