use std::{fmt::Debug, time::Duration};

use actix::{Actor, Context, Handler};
use anyhow::Error;

use crate::experiment::{cache::CacheStatusMessage, wapm::TestCase};

#[derive(Debug)]
pub(crate) struct ProgressMonitor(Box<dyn Progress>);

impl ProgressMonitor {
    pub fn new(progress: Box<dyn Progress>) -> Self {
        ProgressMonitor(progress)
    }
}

pub trait Progress: Debug {
    fn downloading_assets_failed(&mut self, _test_case: TestCase, _error: Error) {}
    fn downloading(&mut self, _test_case: TestCase) {}
    fn cache_hit(&mut self, _test_case: TestCase) {}
    fn cache_miss(&mut self, _test_case: TestCase, _duration: Duration) {}
}

impl Actor for ProgressMonitor {
    type Context = Context<Self>;
}

impl Handler<CacheStatusMessage> for ProgressMonitor {
    type Result = ();

    fn handle(&mut self, msg: CacheStatusMessage, _ctx: &mut Self::Context) {
        match msg {
            CacheStatusMessage::Fetching(test_case) => self.0.downloading(test_case),
            CacheStatusMessage::CacheHit(test_case) => self.0.cache_hit(test_case),
            CacheStatusMessage::CacheMiss {
                test_case,
                duration,
            } => self.0.cache_miss(test_case, duration),
            CacheStatusMessage::DownloadFailed { test_case, error } => {
                self.0.downloading_assets_failed(test_case, error)
            }
        }
    }
}
