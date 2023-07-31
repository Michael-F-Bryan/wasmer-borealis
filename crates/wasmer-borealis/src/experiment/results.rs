use std::{path::PathBuf, time::Duration};

use anyhow::Error;

use crate::{config::Experiment, registry::queries::PackageVersion};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Results {
    pub experiment: Experiment,
    pub reports: Vec<Report>,
    pub total_time: Duration,
    pub experiment_dir: PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Report {
    pub display_name: String,
    pub package_version: PackageVersion,
    pub outcome: Outcome,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    Completed {
        status: ExitStatus,
        run_time: Duration,
        base_dir: PathBuf,
    },
    FetchFailed {
        error: SerializableError,
    },
    SetupFailed {
        base_dir: PathBuf,
        error: SerializableError,
    },
    SpawnFailed {
        base_dir: PathBuf,
        error: SerializableError,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SerializableError {
    pub error: String,
    pub detailed_error: String,
    pub causes: Vec<String>,
}

impl SerializableError {
    fn from_error(error: &Error) -> Self {
        SerializableError {
            error: error.to_string(),
            detailed_error: format!("{error:?}"),
            causes: {
                std::iter::successors(error.source(), |e| e.source())
                    .map(|e| e.to_string())
                    .collect()
            },
        }
    }
}

impl From<Error> for SerializableError {
    fn from(error: Error) -> Self {
        SerializableError::from_error(&error)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExitStatus {
    pub success: bool,
    pub code: i32,
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(value: std::process::ExitStatus) -> Self {
        ExitStatus {
            success: value.success(),
            code: value.code().unwrap_or(1),
        }
    }
}
