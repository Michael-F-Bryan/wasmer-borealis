use std::{path::PathBuf, process::ExitStatus, sync::Arc, time::Duration};

use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, StreamHandler, System, WrapFuture,
};
use anyhow::Error;
use futures::{channel::oneshot::Sender, FutureExt};
use reqwest::Client;

use crate::{
    config::Experiment,
    experiment::{
        cache::{Assets, Cache, FetchAssets},
        runner::{BeginTest, Runner},
        wapm::{FetchTestCases, TestCase, TestCasesDiscovered, Wapm},
    },
    registry::queries::PackageVersion,
};

/// The top-level experiment runner.
#[derive(Debug)]
pub(crate) struct Orchestrator {
    cache: Addr<Cache>,
    experiment: Arc<Experiment>,
    reports: Vec<Report>,
    sender: Option<Sender<Results>>,
    client: Client,
    endpoint: String,
}

impl Orchestrator {
    pub fn new(
        experiment: Arc<Experiment>,
        cache: Addr<Cache>,
        client: Client,
        endpoint: String,
        sender: Sender<Results>,
    ) -> Self {
        Orchestrator {
            cache,
            client,
            endpoint,
            experiment,
            sender: Some(sender),
            reports: Vec::new(),
        }
    }
}

impl Actor for Orchestrator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let wapm = Wapm::new(self.client.clone(), self.endpoint.clone()).start();

        // Kick everything off by telling WAPM to send all the candidates
        // to our orchestrator
        ctx.spawn(
            wapm.send(FetchTestCases {
                filters: self.experiment.filters.clone(),
                recipient: ctx.address().recipient(),
            })
            .map(|_| {})
            .into_actor(self),
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        let results = Results {
            outcomes: std::mem::take(&mut self.reports),
        };
        let _ = self.sender.take().unwrap().send(results);
        System::current().stop();
    }
}

impl Handler<TestCasesDiscovered> for Orchestrator {
    type Result = ();

    fn handle(&mut self, msg: TestCasesDiscovered, ctx: &mut Self::Context) {
        let TestCasesDiscovered(test_cases) = msg;
        let cache = self.cache.clone();

        ctx.spawn(
            cache
                .send(FetchAssets { test_cases })
                .into_actor(self)
                .then(|result, _, ctx| {
                    if let Ok(fetched) = result {
                        ctx.add_stream(futures::stream::iter(fetched.0));
                    }
                    actix::fut::ready(())
                }),
        );
    }
}

impl StreamHandler<(TestCase, Assets)> for Orchestrator {
    fn handle(&mut self, (test_case, assets): (TestCase, Assets), ctx: &mut Self::Context) {
        let runner = Runner::new(self.experiment.clone()).start();
        let addr = ctx.address();

        ctx.spawn(
            runner
                .send(BeginTest { test_case, assets })
                .into_actor(self)
                .then(|result, actor, _| {
                    async move {
                        if let Ok(report) = dbg!(result) {
                            let _ = addr.send(SaveReport(report)).await;
                        }
                    }
                    .into_actor(actor)
                }),
        );
    }
}

#[derive(Debug, actix::Message)]
#[rtype(result = "()")]
struct SaveReport(Report);

impl Handler<SaveReport> for Orchestrator {
    type Result = ();

    fn handle(&mut self, msg: SaveReport, _ctx: &mut Self::Context) {
        self.reports.push(msg.0);
    }
}

#[derive(Debug)]
pub struct Results {
    pub outcomes: Vec<Report>,
}

#[derive(Debug)]
pub struct Report {
    pub display_name: String,
    pub package_version: PackageVersion,
    pub base_dir: PathBuf,
    pub outcome: Outcome,
}

#[derive(Debug)]
pub enum Outcome {
    Completed {
        status: ExitStatus,
        run_time: Duration,
    },
    SetupFailed(Error),
    SpawnFailed(Error),
}
