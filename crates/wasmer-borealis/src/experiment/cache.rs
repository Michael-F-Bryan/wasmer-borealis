use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{Actor, Context, Handler, Recipient};
use anyhow::{Context as _, Error};
use reqwest::Client;
use tempfile::TempDir;
use tokio::sync::Semaphore;

use crate::experiment::wapm::TestCase;

const MAX_CONCURRENT_DOWNLOADS: usize = 16;

#[derive(Debug, Clone)]
pub(crate) struct Cache {
    dir: PathBuf,
    client: Client,
    progress: Recipient<CacheStatusMessage>,
    download_limiter: Arc<Semaphore>,
}

impl Cache {
    pub(crate) fn new(
        dir: PathBuf,
        client: Client,
        progress: Recipient<CacheStatusMessage>,
    ) -> Self {
        Cache {
            dir,
            client,
            progress,
            download_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
        }
    }
}

impl Actor for Cache {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "Result<AssetsFetched, Error>")]
pub(crate) struct FetchAssets {
    pub test_case: TestCase,
}

impl Handler<FetchAssets> for Cache {
    type Result = actix::ResponseFuture<Result<AssetsFetched, Error>>;

    fn handle(
        &mut self,
        msg: FetchAssets,
        _ctx: &mut Self::Context,
    ) -> actix::ResponseFuture<Result<AssetsFetched, Error>> {
        let FetchAssets { test_case } = msg;
        let progress = self.progress.clone();
        let dir = self.dir.clone();
        let client = self.client.clone();
        let semaphore = self.download_limiter.clone();

        Box::pin(async move {
            let _guard = semaphore.acquire().await?;
            let assets = download(&client, &dir, &test_case, progress).await?;
            Ok(AssetsFetched { test_case, assets })
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AssetsFetched {
    pub test_case: TestCase,
    pub assets: Assets,
}

#[derive(Debug, Clone)]
pub(crate) struct Assets {
    pub tarball: PathBuf,
    pub webc: Option<PathBuf>,
}

/// Messages emitted by the [`Cache`] as it downloads a packages.
#[derive(Debug, actix::Message)]
#[rtype(result = "()")]
pub(crate) enum CacheStatusMessage {
    Fetching(TestCase),
    CacheHit(TestCase),
    CacheMiss {
        test_case: TestCase,
        duration: Duration,
    },
}

#[tracing::instrument(skip_all, fields(
        pkg.namespace=test_case.namespace.as_str(),
        pkg.name=test_case.package_name.as_str(),
        pkg.version=test_case.version(),
    ))]
async fn download(
    client: &Client,
    dir: &Path,
    test_case: &TestCase,
    progress: Recipient<CacheStatusMessage>,
) -> Result<Assets, Error> {
    let _ = progress
        .send(CacheStatusMessage::Fetching(test_case.clone()))
        .await;

    let cache_dir = package_version_dir(dir, test_case);
    let tarball_path = cache_dir
        .join(&test_case.package_name)
        .with_extension("tar.gz");
    let webc_path = cache_dir
        .join(&test_case.package_name)
        .with_extension("webc");

    if cache_dir.exists() && tarball_path.exists() {
        tracing::debug!(cache_dir=%cache_dir.display(), "Cache hit!");
        let _ = progress
            .send(CacheStatusMessage::CacheHit(test_case.clone()))
            .await;

        return Ok(Assets {
            tarball: tarball_path,
            webc: webc_path.exists().then_some(webc_path),
        });
    }

    let start = Instant::now();
    let result = do_download(client, dir, &cache_dir, tarball_path, webc_path, test_case).await;

    if result.is_ok() {
        let duration = start.elapsed();
        let _ = progress
            .send(CacheStatusMessage::CacheMiss {
                test_case: test_case.clone(),
                duration,
            })
            .await;
    }

    result
}

async fn do_download(
    client: &Client,
    dir: &Path,
    cache_dir: &Path,
    tarball_path: PathBuf,
    webc_path: PathBuf,
    test_case: &TestCase,
) -> Result<Assets, Error> {
    tokio::fs::create_dir_all(dir)
        .await
        .with_context(|| format!("Unable to create \"{}\"", dir.display()))?;
    let temp = TempDir::new_in(dir).context("Unable to create a temporary directory")?;

    // Download our files to a temporary directory
    download_file(
        client,
        test_case.tarball_url(),
        temp.path().join(tarball_path.file_name().unwrap()),
    )
    .await
    .with_context(|| format!("Downloading \"{}\" failed", test_case.tarball_url()))?;
    if let Some(url) = test_case.webc_url() {
        download_file(
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
            let error =
                Error::new(e).context(format!("Unable to remove \"{}\"", cache_dir.display()));
            return Err(error);
        }
    }

    if let Some(parent) = cache_dir.parent() {
        if let Err(err) = tokio::fs::create_dir_all(parent).await {
            let err = Error::new(err).context(format!("Unable to create \"{}\"", parent.display()));
            return Err(err);
        }
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

        let error = Error::new(e).context(format!(
            "Unable to persist \"{}\" to \"{}\"",
            temp.display(),
            cache_dir.display()
        ));
        return Err(error);
    }

    Ok(Assets {
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
async fn download_file(client: &Client, url: &str, dest: impl AsRef<Path>) -> Result<(), Error> {
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

pub fn package_version_dir(dir: &Path, test_case: &TestCase) -> PathBuf {
    dir.join(&test_case.namespace)
        .join(format!(
            "{}-{}",
            test_case.package_name,
            test_case.package_version.id.inner()
        ))
        .join(test_case.version())
}
