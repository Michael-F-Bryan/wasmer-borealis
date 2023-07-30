use std::{fmt::Debug, path::PathBuf, sync::Arc};

use actix::{Actor, System};
use anyhow::Error;
use reqwest::Client;
use tokio::runtime::Runtime;

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
    endpoint: String,
}

impl ExperimentBuilder {
    pub fn new(experiment: Experiment) -> Self {
        ExperimentBuilder {
            experiment: Arc::new(experiment),
            runtime: None,
            progress: Box::new(Noop),
            cache_dir: None,
            client: None,
            endpoint: PRODUCTION_ENDPOINT.to_string(),
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

    pub fn with_endpoint(self, endpoint: impl Into<String>) -> Self {
        ExperimentBuilder {
            endpoint: endpoint.into(),
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
        } = self;

        let client = client.unwrap_or_default();
        let cache_dir = cache_dir.unwrap_or_else(|| crate::DIRS.cache_dir().to_path_buf());

        let system = match runtime {
            Some(rt) => System::with_tokio_rt(rt),
            None => System::new(),
        };

        let results = system.block_on(async {
            let progress = ProgressMonitor::new(progress).start();
            let cache = Cache::new(cache_dir, client.clone(), progress.recipient()).start();
            let orchestrator = Orchestrator::new(cache, client, endpoint).start();

            orchestrator.send(BeginExperiment { experiment }).await
        })?;

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
            client,
            endpoint,
        } = self;

        f.debug_struct("ExperimentBuilder")
            .field("experiment", experiment)
            .field("progress", progress)
            .field("cache_dir", cache_dir)
            .field("client", client)
            .field("endpoint", endpoint)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy)]
struct Noop;

impl Progress for Noop {}
