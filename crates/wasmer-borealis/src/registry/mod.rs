use anyhow::{Context, Error};
use cynic::{GraphQlResponse, QueryBuilder};
use futures::{Sink, SinkExt};
use reqwest::Client;

#[tracing::instrument(skip_all, fields(namespace))]
pub async fn all_packages_in_namespace<S>(
    client: &Client,
    graphql_endpoint: &str,
    namespace: &str,
    mut dest: S,
) -> Result<(), Error>
where
    S: Sink<queries::Package> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let mut offset = 0;

    loop {
        let op = queries::GetNamespace::build(queries::GetNamespaceVariables {
            name: namespace,
            offset,
        });

        tracing::debug!(offset, "Fetching a page of packages");

        let response: GraphQlResponse<queries::GetNamespace> = client
            .post(graphql_endpoint)
            .header("Content-Type", "application/json")
            .json(&op)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                todo!("Handle errors: {errors:?}");
            }
        }

        let packages: Vec<_> = response
            .data
            .context("Invalid query")?
            .get_namespace
            .with_context(|| format!("Unknown namespace, \"{namespace}\""))?
            .packages
            .edges
            .into_iter()
            .flatten()
            .flat_map(|edge| edge.node)
            .collect();

        if packages.is_empty() {
            break;
        }

        for package in packages {
            dest.send(package).await?;
            offset += 1;
        }

        dest.flush().await?;
    }

    Ok(())
}

#[cynic::schema_for_derives(
    file = "src/registry/schema.graphql",
    module = "crate::registry::schema"
)]
#[allow(unused)]
pub mod queries {

    #[derive(cynic::QueryVariables, Debug, Clone)]
    pub struct GetNamespaceVariables<'a> {
        pub name: &'a str,
        pub offset: i32,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(graphql_type = "Query", variables = "GetNamespaceVariables")]
    pub struct GetNamespace {
        #[arguments(name: $name)]
        pub get_namespace: Option<Namespace>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(variables = "GetNamespaceVariables")]
    pub struct Namespace {
        #[arguments(offset: $offset)]
        pub packages: PackageConnection,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct PackageConnection {
        pub edges: Vec<Option<PackageEdge>>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct PackageEdge {
        pub node: Option<Package>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct Package {
        pub id: cynic::Id,
        pub package_name: String,
        pub namespace: String,
        pub display_name: String,
        pub last_version: Option<PackageVersion>,
        pub versions: Vec<Option<PackageVersion>>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct PackageVersion {
        pub id: cynic::Id,
        pub version: String,
        pub distribution: PackageDistribution,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    pub struct PackageDistribution {
        pub download_url: String,
        pub pirita_download_url: Option<String>,
    }
}

#[allow(non_snake_case, non_camel_case_types)]
mod schema {
    cynic::use_schema!("src/registry/schema.graphql");
}
