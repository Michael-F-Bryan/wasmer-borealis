use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use actix::{Actor, Context, Handler};
use anyhow::{Context as _, Error};
use tokio::sync::Semaphore;

use crate::{
    config::Experiment,
    experiment::{cache::Assets, Outcome, Report, TestCase},
};

#[derive(Debug, Clone)]
pub(crate) struct Runner {
    experiment: Arc<Experiment>,
    semaphore: Arc<Semaphore>,
    base_dir: PathBuf,
}

impl Runner {
    pub(crate) fn new(experiment: Arc<Experiment>, base_dir: PathBuf) -> Self {
        Runner {
            experiment,
            base_dir,
            semaphore: Arc::new(Semaphore::new(
                std::thread::available_parallelism()
                    .unwrap_or(NonZeroUsize::new(4).unwrap())
                    .get(),
            )),
        }
    }
}

impl Actor for Runner {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "Report")]
pub(crate) struct BeginTest {
    pub test_case: TestCase,
    pub assets: Assets,
}

impl Handler<BeginTest> for Runner {
    type Result = actix::ResponseFuture<Report>;

    fn handle(&mut self, msg: BeginTest, _ctx: &mut Self::Context) -> Self::Result {
        let BeginTest { test_case, assets } = msg;

        let base_dir = self
            .base_dir
            .join(&test_case.namespace)
            .join(&test_case.package_name)
            .join(test_case.version());

        let experiment = self.experiment.clone();
        let semaphore = self.semaphore.clone();

        Box::pin(async move {
            let _guard = semaphore.acquire().await.unwrap();
            run_experiment(&experiment, &test_case, &assets, base_dir).await
        })
    }
}

#[tracing::instrument(
    skip_all,
    fields(
        %test_case.namespace,
        %test_case.package_name,
        test_case.version=test_case.version(),
        base_dir=%base_dir.display(),
    )
)]
async fn run_experiment(
    experiment: &Experiment,
    test_case: &TestCase,
    assets: &Assets,
    base_dir: PathBuf,
) -> Report {
    let dirs = directories::BaseDirs::new().unwrap();

    let mut cmd = match setup(experiment, test_case, assets, &base_dir, dirs.home_dir()).await {
        Ok(cmd) => cmd,
        Err(error) => {
            return Report {
                display_name: test_case.display_name(),
                package_version: test_case.package_version.clone(),
                outcome: Outcome::SetupFailed {
                    base_dir,
                    error: error.into(),
                },
            }
        }
    };

    tracing::debug!(cmd=?cmd.as_std(), "Invoking wasmer CLI");
    let start = Instant::now();

    let outcome = match cmd.status().await {
        Ok(status) => Outcome::Completed {
            base_dir,
            status: status.into(),
            run_time: start.elapsed(),
        },
        Err(error) => {
            let error = Error::new(error).context(format!(
                "Unable to start \"{}\", is it installed?",
                cmd.as_std().get_program().to_string_lossy()
            ));
            Outcome::SetupFailed {
                error: error.into(),
                base_dir,
            }
        }
    };

    Report {
        display_name: test_case.display_name(),
        package_version: test_case.package_version.clone(),
        outcome,
    }
}

#[tracing::instrument(skip_all)]
async fn setup(
    experiment: &Experiment,
    test_case: &TestCase,
    assets: &Assets,
    base_dir: &Path,
    home_dir: &Path,
) -> Result<tokio::process::Command, Error> {
    if base_dir.exists() {
        tokio::fs::remove_dir_all(base_dir)
            .await
            .context("Unable to remove the base directory")?;
    }
    tokio::fs::create_dir_all(base_dir)
        .await
        .context("Unable to create the base dir")?;

    let test_case_json = base_dir.join("test_case.json");
    let json = serde_json::to_string_pretty(test_case)?;
    tokio::fs::write(test_case_json, json).await?;

    let working_dir = base_dir.join("working");

    tokio::fs::create_dir_all(&working_dir)
        .await
        .context("Unable to clean the working dir")?;

    let tarball_path = working_dir.join("package.tar.gz");
    tokio::fs::symlink(&assets.tarball, &tarball_path)
        .await
        .context("Unable to create the tarball symlink")?;

    let webc_path = working_dir.join("package.webc");
    if let Some(webc) = &assets.webc {
        tokio::fs::symlink(webc, &webc_path)
            .await
            .context("Unable to create the webc symlink")?;
    }

    let env = Env::new(working_dir.clone(), test_case);

    let mut cmd = tokio::process::Command::new("wasmer");

    let stdout = tokio::fs::File::create(base_dir.join("stdout.txt"))
        .await
        .context("Unable to open stdout.txt")?;
    let stderr = tokio::fs::File::create(base_dir.join("stderr.txt"))
        .await
        .context("Unable to open stderr.txt")?;

    cmd.current_dir(base_dir)
        .stdout(stdout.into_std().await)
        .stderr(stderr.into_std().await)
        .stdin(std::process::Stdio::null())
        .env_clear();

    let whitelisted_vars = ["PATH", "WASMER_DIR"];

    for var in whitelisted_vars {
        if let Some(value) = std::env::var_os(var) {
            cmd.env(var, value);
        }
    }

    for (name, value) in &experiment.wasmer.env {
        let value = value.resolve(home_dir, |var| env.get_host(var));
        cmd.env(name, value.as_ref());
    }

    cmd.arg("run").arg(&experiment.package);

    for arg in &experiment.wasmer.args {
        let arg = arg.resolve(home_dir, |var| env.get_host(var));
        cmd.arg(arg.as_ref());
    }

    for (name, value) in &experiment.env {
        let value = value.resolve(home_dir, |var| env.get_guest(var));
        cmd.arg(format!("--env={name}={value}"));
    }

    cmd.arg("--");

    for arg in &experiment.args {
        let arg = arg.resolve(home_dir, |var| env.get_guest(var));
        cmd.arg(arg.as_ref());
    }

    Ok(cmd)
}

#[derive(Debug, PartialEq, Clone)]
struct Env {
    common: HashMap<&'static str, String>,
    host: HashMap<&'static str, String>,
}

impl Env {
    fn new(working_dir: PathBuf, test_case: &TestCase) -> Self {
        let mut common: HashMap<&str, String> = HashMap::new();

        common.insert("PKG_NAMESPACE", test_case.namespace.clone());
        common.insert("PKG_NAME", test_case.package_name.clone());
        common.insert("PKG_VERSION", test_case.version().to_string());
        common.insert("TARBALL_FILENAME", "package.tar.gz".to_string());

        let mut host: HashMap<&str, String> = HashMap::new();

        host.insert(
            "TARBALL_PATH",
            working_dir.join("package.tar.gz").display().to_string(),
        );

        if test_case.webc_url().is_some() {
            host.insert(
                "WEBC_PATH",
                working_dir.join("package.webc").display().to_string(),
            );
            common.insert("WEBC_FILENAME", "package.webc".to_string());
        }

        host.insert("WORKING_DIR", working_dir.display().to_string());

        Env { common, host }
    }

    fn get_host(&self, var: &str) -> Option<String> {
        self.host.get(var).or_else(|| self.common.get(var)).cloned()
    }

    fn get_guest(&self, var: &str) -> Option<String> {
        self.common.get(var).cloned()
    }
}
