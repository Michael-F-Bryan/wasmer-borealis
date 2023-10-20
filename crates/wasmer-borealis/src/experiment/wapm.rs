use actix::{Actor, AsyncContext, Context, Handler, WrapFuture};
use futures::{channel::mpsc::Sender, SinkExt, Stream, StreamExt};
use reqwest::Client;
use tracing::Instrument;
use url::Url;

use crate::{
    config::Filters,
    registry::queries::{Package, PackageVersion},
};

#[derive(Debug, Clone)]
pub(crate) struct Wapm {
    client: Client,
    endpoint: Url,
}

impl Wapm {
    /// Initialize the [`Wapm`] actor.
    ///
    /// # Authentication
    ///
    /// If you want access to all packages, you will need to make sure the
    /// [`Client`] has been configured to send the right `Authorization` header.
    pub fn new(client: Client, endpoint: Url) -> Self {
        Wapm { client, endpoint }
    }
}

impl Actor for Wapm {
    type Context = Context<Self>;
}

/// Tell [`Wapm`] to start looking for all [`TestCase`]s that should be
/// included in the experiment.
#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "()")]
pub(crate) struct FetchTestCases {
    pub filters: Filters,
    pub recipient: Sender<TestCaseDiscovered>,
}

/// A batch of [`TestCase`]s have been discovered from the registry.
#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "()")]
pub(crate) struct TestCaseDiscovered(pub TestCase);

impl Handler<FetchTestCases> for Wapm {
    type Result = ();

    fn handle(&mut self, msg: FetchTestCases, ctx: &mut Self::Context) {
        let FetchTestCases {
            filters,
            mut recipient,
        } = msg;

        let client = self.client.clone();
        let endpoint = self.endpoint.clone();

        ctx.spawn(
            async move {
                let mut responses = discover_test_cases(client, filters, endpoint);

                while let Some(test_cases) = responses.next().await {
                    for test_case in test_cases {
                        if recipient.send(TestCaseDiscovered(test_case)).await.is_err() {
                            break;
                        };
                    }
                }
            }
            .instrument(tracing::debug_span!("discover_test_cases"))
            .into_actor(self),
        );
    }
}

/// Discover [`TestCase`]s, retrieving them page-by-page.
fn discover_test_cases(
    client: Client,
    filters: Filters,
    endpoint: Url,
) -> impl Stream<Item = Vec<TestCase>> {
    let (mut sender, receiver) = futures::channel::mpsc::channel(1);
    let Filters {
        namespaces,
        blacklist,
        include_every_version,
        users,
    } = filters;

    let hostname = endpoint.host_str().unwrap_or("unknown").to_string();

    if namespaces.is_empty() && users.is_empty() {
        tokio::spawn(async move {
            if let Err(e) =
                crate::registry::all_packages(&client, endpoint.as_str(), &mut sender).await
            {
                tracing::error!(error = &*e, "Unable to list all packages");
            }
        });
    } else {
        tokio::spawn(async move {
            for namespace in &namespaces {
                if let Err(e) = crate::registry::all_packages_in_namespace(
                    &client,
                    endpoint.as_str(),
                    namespace,
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

            for user in &users {
                if let Err(e) = crate::registry::all_packages_by_user(
                    &client,
                    endpoint.as_str(),
                    user,
                    &mut sender,
                )
                .await
                {
                    tracing::error!(
                        error = &*e,
                        user = user.as_str(),
                        "Unable to fetch a user's packages"
                    );
                }
            }
        });
    }

    receiver.map(move |page| {
        page.into_iter()
            .filter(|pkg| blacklist.is_empty() || !blacklist.contains(&pkg.display_name))
            .flat_map(|pkg| {
                if include_every_version {
                    TestCase::all(&hostname, pkg)
                } else {
                    TestCase::latest(&hostname, pkg)
                }
            })
            .collect()
    })
}

/// A package version that will be included in the experiment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestCase {
    /// The hostname of the registry this [`TestCase`] came from.
    pub registry: String,
    /// The namespace or user that owns the package.
    pub namespace: String,
    /// The package's name.
    pub package_name: String,
    pub package_version: PackageVersion,
}

impl TestCase {
    fn all(registry_hostname: &str, pkg: Package) -> Vec<TestCase> {
        pkg.versions
            .into_iter()
            .flatten()
            .map(|version| {
                TestCase::new(
                    registry_hostname,
                    pkg.namespace.clone(),
                    pkg.package_name.clone(),
                    version,
                )
            })
            .collect()
    }

    fn latest(registry: &str, pkg: Package) -> Vec<TestCase> {
        if let Some(version) = pkg.last_version {
            vec![TestCase::new(
                registry,
                pkg.namespace,
                pkg.package_name,
                version,
            )]
        } else {
            Vec::new()
        }
    }

    fn new(
        registry_hostname: &str,
        namespace: String,
        package_name: String,
        package_version: PackageVersion,
    ) -> Self {
        TestCase {
            registry: registry_hostname.to_string(),
            namespace,
            package_name,
            package_version,
        }
    }

    pub fn tarball_url(&self) -> &str {
        &self.package_version.distribution.download_url
    }

    pub fn webc_url(&self) -> Option<&str> {
        self.package_version
            .distribution
            .pirita_download_url
            .as_deref()
    }

    pub fn version(&self) -> &str {
        &self.package_version.version
    }

    pub fn display_name(&self) -> String {
        format!("{}/{}", self.namespace, self.package_name)
    }
}
