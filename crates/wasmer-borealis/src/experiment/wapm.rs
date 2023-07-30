use actix::{Actor, AsyncContext, Context, Handler, Recipient, WrapFuture};
use futures::{Stream, StreamExt};
use reqwest::Client;
use tracing::Instrument;

use crate::{
    config::Filters,
    registry::queries::{Package, PackageVersion},
};

#[derive(Debug, Clone)]
pub(crate) struct Wapm {
    client: Client,
    endpoint: String,
}

impl Wapm {
    pub fn new(client: Client, endpoint: String) -> Self {
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
    pub recipient: Recipient<TestCasesDiscovered>,
}

/// A batch of [`TestCase`]s have been discovered from the registry.
#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "()")]
pub(crate) struct TestCasesDiscovered(pub Vec<TestCase>);

impl Handler<FetchTestCases> for Wapm {
    type Result = ();

    fn handle(&mut self, msg: FetchTestCases, ctx: &mut Self::Context) {
        let FetchTestCases { filters, recipient } = msg;

        let client = self.client.clone();
        let endpoint = self.endpoint.clone();

        ctx.spawn(
            async move {
                let mut responses = discover_test_cases(client, filters, endpoint);

                while let Some(test_case) = responses.next().await {
                    if recipient
                        .send(TestCasesDiscovered(test_case))
                        .await
                        .is_err()
                    {
                        break;
                    };
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
    endpoint: String,
) -> impl Stream<Item = Vec<TestCase>> {
    let (mut sender, receiver) = futures::channel::mpsc::channel(1);
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
                if let Err(e) = crate::registry::all_packages_in_namespace(
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

    receiver.map(move |page| {
        page.into_iter()
            .filter(|pkg| blacklist.is_empty() || !blacklist.contains(&pkg.display_name))
            .flat_map(|pkg| {
                if include_every_version {
                    TestCase::all(pkg)
                } else {
                    TestCase::latest(pkg)
                }
            })
            .collect()
    })
}

/// A package version that will be included in the experiment.
#[derive(Debug, Clone)]
pub struct TestCase {
    pub namespace: String,
    pub package_name: String,
    pub package_version: PackageVersion,
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
