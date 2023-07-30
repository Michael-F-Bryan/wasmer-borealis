use anyhow::{Context, Error};
use cynic::{GraphQlResponse, Operation, QueryBuilder};
use futures::{Sink, SinkExt};
use reqwest::Client;

use crate::registry::queries::Variables;

#[tracing::instrument(skip_all, fields(username))]
pub async fn all_packages_by_user<S>(
    client: &Client,
    graphql_endpoint: &str,
    username: &str,
    dest: S,
) -> Result<(), Error>
where
    S: Sink<Vec<queries::Package>> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    packages_query(
        client,
        graphql_endpoint,
        dest,
        |offset| {
            queries::GetUserPackages::build(Variables {
                name: username,
                offset,
            })
        },
        |result| {
            let user = result
                .get_user
                .with_context(|| format!("Unknown user, \"{username}\""))?;
            Ok(user.packages)
        },
    )
    .await
}

#[tracing::instrument(skip_all, fields(namespace))]
pub async fn all_packages_in_namespace<S>(
    client: &Client,
    graphql_endpoint: &str,
    namespace: &str,
    dest: S,
) -> Result<(), Error>
where
    S: Sink<Vec<queries::Package>> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    packages_query(
        client,
        graphql_endpoint,
        dest,
        |offset| {
            queries::GetNamespace::build(Variables {
                name: namespace,
                offset,
            })
        },
        |result| {
            let ns = result
                .get_namespace
                .with_context(|| format!("Unknown namespace, \"{namespace}\""))?;
            Ok(ns.packages)
        },
    )
    .await
}

#[tracing::instrument(skip_all, fields(namespace))]
pub async fn packages_query<'a, S, Q, Build, GetPackages>(
    client: &Client,
    graphql_endpoint: &str,
    mut dest: S,
    build: Build,
    get_packages: GetPackages,
) -> Result<(), Error>
where
    S: Sink<Vec<queries::Package>> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    Build: Fn(i32) -> Operation<Q, Variables<'a>>,
    GetPackages: Fn(Q) -> Result<queries::PackageConnection, Error>,
    Q: serde::de::DeserializeOwned,
{
    let mut offset = 0;

    loop {
        let op = build(offset);

        tracing::debug!(offset, "Fetching a page of packages");

        let response: GraphQlResponse<Q> = client
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

        let query_result = response.data.context("Invalid query")?;
        let packages: Vec<_> = get_packages(query_result)?
            .edges
            .into_iter()
            .flatten()
            .flat_map(|edge| edge.node)
            .collect();

        if packages.is_empty() {
            break;
        }

        offset += i32::try_from(packages.len()).unwrap();
        dest.send(packages).await?;
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
    pub struct Variables<'a> {
        pub name: &'a str,
        pub offset: i32,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(graphql_type = "Query", variables = "Variables")]
    pub struct GetUserPackages {
        #[arguments(username: $name)]
        pub get_user: Option<User>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(variables = "Variables")]
    pub struct User {
        #[arguments(offset: $offset)]
        pub packages: PackageConnection,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(graphql_type = "Query", variables = "Variables")]
    pub struct GetNamespace {
        #[arguments(name: $name)]
        pub get_namespace: Option<Namespace>,
    }

    #[derive(cynic::QueryFragment, Debug, Clone)]
    #[cynic(variables = "Variables")]
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
