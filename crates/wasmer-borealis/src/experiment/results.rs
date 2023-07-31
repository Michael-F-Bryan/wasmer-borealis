use std::{path::PathBuf, process::ExitStatus, time::Duration};

use anyhow::Error;

use crate::registry::queries::PackageVersion;

#[derive(Default, Debug)]
pub struct Results {
    pub reports: Vec<Report>,
    pub total_time: Duration,
    pub experiment_dir: PathBuf,
}

impl Extend<Report> for Results {
    fn extend<T: IntoIterator<Item = Report>>(&mut self, iter: T) {
        self.reports.extend(iter);
    }
}

#[derive(Debug)]
pub struct Report {
    pub display_name: String,
    pub package_version: PackageVersion,
    pub outcome: Outcome,
}

#[derive(Debug)]
pub enum Outcome {
    Completed {
        status: ExitStatus,
        run_time: Duration,
    },
    FetchFailed {
        error: Error,
    },
    SetupFailed {
        base_dir: PathBuf,
        error: Error,
    },
    SpawnFailed {
        base_dir: PathBuf,
        error: Error,
    },
}
