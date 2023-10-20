use std::{fmt::Debug, path::PathBuf, sync::Arc};

use actix::{Actor, System};
use anyhow::Error;
use reqwest::Client;
use tokio::runtime::Runtime;
use tracing::Instrument;
use url::Url;

use crate::{
    config::Experiment,
    experiment::{
        cache::Cache,
        orchestrator::{BeginExperiment, Orchestrator},
        progress::{Progress, ProgressMonitor},
        Results,
    },
};

const PRODUCTION_ENDPOINT: &str = "https://registry.wasmer.io/graphql";

#[must_use = "An ExperimentBuilder won't do anything unless you call run()"]
pub struct ExperimentBuilder {
    experiment: Arc<Experiment>,
    runtime: Option<Box<dyn Fn() -> Runtime>>,
    progress: Box<dyn Progress>,
    cache_dir: Option<PathBuf>,
    client: Option<Client>,
    endpoint: Url,
    experiment_dir: Option<PathBuf>,
}

impl ExperimentBuilder {
    pub fn new(experiment: Experiment) -> Self {
        ExperimentBuilder {
            experiment: Arc::new(experiment),
            runtime: None,
            progress: Box::new(Noop),
            cache_dir: None,
            client: None,
            endpoint: PRODUCTION_ENDPOINT.parse().unwrap(),
            experiment_dir: None,
        }
    }

    pub fn with_runtime(self, runtime: impl Fn() -> Runtime + 'static) -> Self {
        ExperimentBuilder {
            runtime: Some(Box::new(runtime)),
            ..self
        }
    }

    pub fn with_progress(self, progress: impl Progress + 'static) -> Self {
        ExperimentBuilder {
            progress: Box::new(progress),
            ..self
        }
    }

    pub fn with_client(self, client: Client) -> Self {
        ExperimentBuilder {
            client: Some(client),
            ..self
        }
    }

    pub fn with_endpoint(self, endpoint: impl AsRef<str>) -> Result<Self, url::ParseError> {
        let endpoint = endpoint.as_ref().parse()?;
        Ok(ExperimentBuilder { endpoint, ..self })
    }

    pub fn with_experiment_dir(self, experiment_dir: impl Into<PathBuf>) -> Self {
        ExperimentBuilder {
            experiment_dir: Some(experiment_dir.into()),
            ..self
        }
    }

    pub fn run(self) -> Result<Results, Error> {
        let ExperimentBuilder {
            experiment,
            runtime,
            progress,
            cache_dir,
            client,
            endpoint,
            experiment_dir,
        } = self;

        let client = client.unwrap_or_default();
        let cache_dir = cache_dir.unwrap_or_else(|| crate::DIRS.cache_dir().to_path_buf());
        let experiment_dir = experiment_dir.unwrap_or_else(|| {
            crate::DIRS
                .data_local_dir()
                .join(uuid::Uuid::new_v4().to_string())
        });

        let system = match runtime {
            Some(rt) => System::with_tokio_rt(rt),
            None => System::new(),
        };

        let results = system.block_on(
            async {
                let progress = ProgressMonitor::new(progress).start();
                let cache = Cache::new(cache_dir, client.clone(), progress.recipient()).start();
                let orchestrator = Orchestrator::new(cache, client, endpoint).start();

                orchestrator
                    .send(BeginExperiment {
                        experiment,
                        base_dir: experiment_dir.clone(),
                    })
                    .await
            }
            .in_current_span(),
        )?;

        let report = crate::render::html(&results)?;
        let reports_html = experiment_dir.join("report.html");
        std::fs::write(reports_html, report)?;

        let reports_json = experiment_dir.join("results.json");
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(reports_json, json)?;

        Ok(results)
    }
}

impl Debug for ExperimentBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ExperimentBuilder {
            experiment,
            runtime: _,
            progress,
            cache_dir,
            experiment_dir,
            client,
            endpoint,
        } = self;

        f.debug_struct("ExperimentBuilder")
            .field("experiment", experiment)
            .field("progress", progress)
            .field("cache_dir", cache_dir)
            .field("experiment_dir", experiment_dir)
            .field("client", client)
            .field("endpoint", endpoint)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy)]
struct Noop;

impl Progress for Noop {}
