use std::{path::PathBuf, sync::Arc, time::Instant};

use actix::{Actor, Addr, Context, Handler, ResponseFuture};
use anyhow::Error;
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::Client;
use url::Url;

use crate::{
    config::Experiment,
    experiment::{
        cache::{AssetsFetched, Cache, FetchAssets},
        runner::{BeginTest, Runner},
        wapm::{FetchTestCases, TestCaseDiscovered, Wapm},
        Outcome, Report, Results,
    },
};

/// The top-level experiment runner.
#[derive(Debug)]
pub(crate) struct Orchestrator {
    cache: Addr<Cache>,
    client: Client,
    endpoint: Url,
}

impl Orchestrator {
    pub fn new(cache: Addr<Cache>, client: Client, endpoint: Url) -> Self {
        Orchestrator {
            cache,
            client,
            endpoint,
        }
    }
}

impl Actor for Orchestrator {
    type Context = Context<Self>;
}

#[derive(Debug, actix::Message)]
#[rtype(result = "Results")]
pub struct BeginExperiment {
    pub experiment: Arc<Experiment>,
    /// The directory experiment results should be saved to.
    pub base_dir: PathBuf,
}

impl Handler<BeginExperiment> for Orchestrator {
    type Result = ResponseFuture<Results>;

    fn handle(
        &mut self,
        msg: BeginExperiment,
        _ctx: &mut Self::Context,
    ) -> actix::ResponseFuture<Results> {
        let BeginExperiment {
            experiment,
            base_dir,
        } = msg;
        let start = Instant::now();

        tracing::info!(?base_dir, "Experiment started");

        let (sender, receiver) = futures::channel::mpsc::channel(1);

        let cache = self.cache.clone();
        let wapm = Wapm::new(self.client.clone(), self.endpoint.clone()).start();
        let runner = Runner::new(experiment.clone(), base_dir.join("experiments")).start();

        wapm.do_send(FetchTestCases {
            filters: experiment.filters.clone(),
            recipient: sender,
        });

        let mut reports = receiver.map(move |TestCaseDiscovered(test_case)| {
            let cache = cache.clone();
            let runner = runner.clone();

            async move {
                let result = cache
                    .send(FetchAssets {
                        test_case: test_case.clone(),
                    })
                    .await
                    .map_err(Error::from)
                    .and_then(|r| r);

                let begin_test = match result {
                    Ok(AssetsFetched { test_case, assets }) => BeginTest { test_case, assets },
                    Err(error) => {
                        return Report {
                            display_name: test_case.display_name(),
                            package_version: test_case.package_version,
                            outcome: Outcome::FetchFailed {
                                error: error.into(),
                            },
                        };
                    }
                };

                runner.send(begin_test).await.unwrap()
            }
        });

        Box::pin(async move {
            let mut futures = FuturesUnordered::new();
            let mut completed = Vec::new();

            // Note: for maximum throughput, poll the reports while still
            // fetching test cases.
            loop {
                futures::select! {
                    fut = reports.next() => {
                        match fut {
                            Some(fut) => futures.push(fut),
                            None => {
                                break;
                            },
                        }
                    }
                    report = futures.next() => {
                        if let Some(report) = report {
                            completed.push(report);
                        }
                    }
                }
            }

            let remaining_reports: Vec<_> = futures.collect().await;
            completed.extend(remaining_reports);

            Results {
                experiment: Experiment::clone(&experiment),
                reports: completed,
                total_time: start.elapsed(),
                experiment_dir: base_dir,
            }
        })
    }
}
