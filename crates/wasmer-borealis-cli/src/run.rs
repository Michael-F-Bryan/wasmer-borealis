use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::ExitStatus,
    time::{Duration, Instant},
};

use anyhow::{Context, Error};
use clap::Parser;
use futures::{Stream, StreamExt};
use reqwest::Client;

use tempfile::TempDir;
use wasmer_borealis::{
    config::{Document, Experiment, Filters},
    registry::queries::{Package, PackageVersion},
};

#[derive(Parser, Debug)]
pub struct Run {
    #[clap(long, default_value = "wasmer.io")]
    registry: String,
    #[clap(long)]
    cache: Option<PathBuf>,
    /// The number of test cases to run in parallel.
    #[clap(long, env, default_value = "16")]
    concurrency: usize,
    /// The experiment to run.
    experiment: PathBuf,
}

impl Run {
    #[tracing::instrument(level = "debug", skip_all)]
    #[tokio::main]
    pub async fn execute(self) -> Result<(), Error> {
        let experiment = std::fs::read_to_string(&self.experiment)
            .with_context(|| format!("Unable to read \"{}\"", self.experiment.display()))?;
        let Document { experiment, .. } = serde_json::from_str(&experiment)
            .context("Unable to deserialize the experiment file")?;
        let experiment_name = self
            .experiment
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Unable to determine the experiment name")?;

        let url = format!("https://registry.{}/graphql", self.registry);
        let client = Client::new();
        let cache = self.cache();

        let results: Vec<_> = discover_test_cases(client.clone(), experiment.filters.clone(), url)
            .map(|test_case| {
                let client = client.clone();
                let cache = cache.clone();
                let experiment = experiment.clone();

                let base_dir = crate::DIRS
                    .data_local_dir()
                    .join("experiments")
                    .join(experiment_name)
                    .join(&test_case.namespace)
                    .join(&test_case.package_name)
                    .join(test_case.version());

                async move {
                    let assets = match cache.download(&client, &test_case).await {
                        Ok(assets) => assets,
                        Err(e) => {
                            let err = e.context("Unable to download the test case");
                            return Report {
                                base_dir,
                                outcome: Outcome::SetupFailed(err),
                                display_name: test_case.display_name(),
                                package_version: test_case.package_version,
                            };
                        }
                    };

                    run_experiment(&experiment, &test_case, &assets, base_dir).await
                }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        print_results(results);

        Ok(())
    }

    fn cache(&self) -> Cache {
        Cache {
            dir: self
                .cache
                .clone()
                .unwrap_or_else(|| crate::DIRS.cache_dir().to_path_buf()),
        }
    }
}

fn print_results(reports: Vec<Report>) {
    let mut success = Vec::new();
    let mut failures = Vec::new();
    let mut errors = Vec::new();

    for report in reports {
        match &report.outcome {
            Outcome::Completed { status, .. } if status.success() => success.push(report),
            Outcome::Completed { .. } => failures.push(report),
            Outcome::SetupFailed(_) | Outcome::SpawnFailed(_) => errors.push(report),
        }
    }

    if !failures.is_empty() {
        println!("==== Failures ====");
        println!("{failures:#?}");
        println!();
    }

    if !errors.is_empty() {
        println!("==== Errors ====");
        println!("{errors:#?}");
        println!();
    }

    println!(
        "Success: {}, failures: {}, errors: {}",
        success.len(),
        failures.len(),
        errors.len()
    );
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
    assets: &DownloadedAssets,
    base_dir: PathBuf,
) -> Report {
    let dirs = directories::BaseDirs::new().unwrap();

    let mut cmd = match setup(experiment, test_case, assets, &base_dir, dirs.home_dir()).await {
        Ok(cmd) => cmd,
        Err(e) => {
            return Report {
                display_name: test_case.display_name(),
                package_version: test_case.package_version.clone(),
                base_dir,
                outcome: Outcome::SetupFailed(e),
            }
        }
    };

    tracing::debug!(cmd=?cmd.as_std(), "Invoking wasmer CLI");
    let start = Instant::now();

    let outcome = match cmd.status().await {
        Ok(status) => Outcome::Completed {
            status,
            run_time: start.elapsed(),
        },
        Err(e) => {
            let err = Error::new(e).context(format!(
                "Unable to start \"{}\", is it installed?",
                cmd.as_std().get_program().to_string_lossy()
            ));
            Outcome::SetupFailed(err)
        }
    };

    Report {
        display_name: test_case.display_name(),
        package_version: test_case.package_version.clone(),
        base_dir,
        outcome,
    }
}

#[tracing::instrument(skip_all)]
async fn setup(
    experiment: &Experiment,
    test_case: &TestCase,
    assets: &DownloadedAssets,
    base_dir: &Path,
    home_dir: &Path,
) -> Result<tokio::process::Command, Error> {
    if base_dir.exists() {
        tokio::fs::remove_dir_all(&base_dir)
            .await
            .context("Unable to remove the base directory")?;
    }
    tokio::fs::create_dir_all(&base_dir)
        .await
        .context("Unable to create the base dir")?;

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

#[derive(Debug)]
struct Report {
    display_name: String,
    package_version: PackageVersion,
    base_dir: PathBuf,
    outcome: Outcome,
}

impl Report {
    fn success(&self) -> bool {
        self.outcome.success()
    }
}

#[derive(Debug)]
enum Outcome {
    Completed {
        status: ExitStatus,
        run_time: Duration,
    },
    SetupFailed(Error),
    SpawnFailed(Error),
}

impl Outcome {
    fn success(&self) -> bool {
        matches!(self, Outcome::Completed { status, .. } if status.success())
    }
}

#[derive(Debug, Clone)]
struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn package_version_dir(&self, test_case: &TestCase) -> PathBuf {
        self.dir
            .join(&test_case.namespace)
            .join(&test_case.package_name)
            .join(test_case.version())
    }

    #[tracing::instrument(skip_all, fields(
        pkg.namespace=test_case.namespace.as_str(),
        pkg.name=test_case.package_name.as_str(),
        pkg.version=test_case.version(),
    ))]
    pub async fn download(
        &self,
        client: &Client,
        test_case: &TestCase,
    ) -> Result<DownloadedAssets, Error> {
        let cache_dir = self.package_version_dir(test_case);
        let tarball_path = cache_dir
            .join(&test_case.package_name)
            .with_extension("tar.gz");
        let webc_path = cache_dir
            .join(&test_case.package_name)
            .with_extension("webc");

        if cache_dir.exists() && tarball_path.exists() {
            tracing::debug!(cache_dir=%cache_dir.display(), "Cache hit!");

            return Ok(DownloadedAssets {
                tarball: tarball_path,
                webc: webc_path.exists().then_some(webc_path),
            });
        }

        tokio::fs::create_dir_all(&self.dir)
            .await
            .with_context(|| format!("Unable to create \"{}\"", self.dir.display()))?;
        let temp = TempDir::new_in(&self.dir).context("Unable to create a temporary directory")?;

        // Download our files to a temporary directory
        self.download_file(
            client,
            test_case.tarball_url(),
            temp.path().join(tarball_path.file_name().unwrap()),
        )
        .await
        .with_context(|| format!("Downloading \"{}\" failed", test_case.tarball_url()))?;
        if let Some(url) = test_case.webc_url() {
            self.download_file(
                client,
                url,
                temp.path().join(webc_path.file_name().unwrap()),
            )
            .await
            .with_context(|| format!("Downloading \"{url}\" failed"))?;
        }

        tracing::debug!(
            from=%temp.path().display(),
            to=%cache_dir.display(),
            "Persisting downloaded artifacts",
        );

        // Before persisting the downloaded directory, make sure we remove
        // any existing stuff
        if let Err(e) = tokio::fs::remove_dir_all(&cache_dir).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(
                    Error::new(e).context(format!("Unable to remove \"{}\"", cache_dir.display()))
                );
            }
        }

        if let Some(parent) = cache_dir.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Unable to create \"{}\"", parent.display()))?;
        }

        let temp = temp.into_path();

        // Now we can (mostly atomically) move the cached assets into place
        if let Err(e) = tokio::fs::rename(&temp, &cache_dir).await {
            if let Err(e) = tokio::fs::remove_dir_all(&temp).await {
                tracing::warn!(
                    temp_dir=%temp.display(),
                    dest=%cache_dir.display(),
                    error=&e as &dyn std::error::Error,
                    "Unable to clean up the temporary folder after failing to persist it",
                );
            }

            return Err(Error::new(e).context(format!(
                "Unable to persist \"{}\" to \"{}\"",
                temp.display(),
                cache_dir.display()
            )));
        }

        Ok(DownloadedAssets {
            tarball: tarball_path,
            webc: test_case
                .package_version
                .distribution
                .pirita_download_url
                .is_some()
                .then_some(webc_path),
        })
    }

    #[tracing::instrument(skip_all, fields(url, bytes_read=tracing::field::Empty))]
    async fn download_file(
        &self,
        client: &Client,
        url: &str,
        dest: impl AsRef<Path>,
    ) -> Result<(), Error> {
        let dest = dest.as_ref();
        tracing::debug!(
            dest=%dest.display(),
            url,
            "Downloading",
        );
        let payload = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        tracing::Span::current().record("bytes_read", payload.len());
        tracing::debug!("Download complete");

        tokio::fs::write(dest, payload)
            .await
            .with_context(|| format!("Unable to save to \"{}\"", dest.display()))?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DownloadedAssets {
    tarball: PathBuf,
    webc: Option<PathBuf>,
}

fn discover_test_cases(
    client: Client,
    filters: Filters,
    endpoint: String,
) -> impl Stream<Item = TestCase> {
    let (mut sender, receiver) = futures::channel::mpsc::channel(16);
    let Filters {
        namespaces,
        blacklist,
        include_every_version,
        users,
    } = filters;

    if namespaces.is_empty() && users.is_empty() {
        todo!("Fetch all packages");
    } else {
        tokio::spawn(async move {
            for namespace in &namespaces {
                if let Err(e) = wasmer_borealis::registry::all_packages_in_namespace(
                    &client,
                    &endpoint,
                    "wasmer",
                    &mut sender,
                )
                .await
                {
                    tracing::error!(
                        error = &*e,
                        namespace = namespace.as_str(),
                        "Unable to fetch a namespace's packages"
                    );
                }
            }
        });
    }

    receiver
        .flat_map(futures::stream::iter)
        .filter(move |pkg| {
            futures::future::ready(blacklist.is_empty() || !blacklist.contains(&pkg.display_name))
        })
        .flat_map(move |pkg| {
            futures::stream::iter({
                if include_every_version {
                    TestCase::all(pkg)
                } else {
                    TestCase::latest(pkg)
                }
            })
        })
}

#[derive(Debug, Clone)]
struct TestCase {
    namespace: String,
    package_name: String,
    package_version: PackageVersion,
}

impl TestCase {
    fn all(pkg: Package) -> Vec<TestCase> {
        pkg.versions
            .into_iter()
            .flatten()
            .map(|version| TestCase::new(pkg.namespace.clone(), pkg.package_name.clone(), version))
            .collect()
    }

    fn latest(pkg: Package) -> Vec<TestCase> {
        if let Some(version) = pkg.last_version {
            vec![TestCase::new(pkg.namespace, pkg.package_name, version)]
        } else {
            Vec::new()
        }
    }

    fn new(namespace: String, package_name: String, package_version: PackageVersion) -> Self {
        TestCase {
            namespace,
            package_name,
            package_version,
        }
    }

    fn tarball_url(&self) -> &str {
        &self.package_version.distribution.download_url
    }

    fn webc_url(&self) -> Option<&str> {
        self.package_version
            .distribution
            .pirita_download_url
            .as_deref()
    }

    fn version(&self) -> &str {
        &self.package_version.version
    }

    fn display_name(&self) -> String {
        format!("{}/{}", self.namespace, self.package_name)
    }
}
