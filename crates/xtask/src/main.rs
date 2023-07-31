mod dist;
mod schema;

use std::path::Path;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{dist::Dist, schema::Schema};

fn main() -> Result<(), anyhow::Error> {
    let Args { verbosity, cmd } = Args::parse();

    initialize_logging(verbosity.log_level_filter());

    match cmd {
        Cmd::Dist(d) => d.run(),
        Cmd::Schema(s) => s.run(),
    }
}

#[derive(Debug, clap::Parser)]
#[clap(about, version, author)]
struct Args {
    #[clap(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
enum Cmd {
    /// Generate release artifacts.
    Dist(Dist),
    /// Update the GraphQL schema.
    Schema(Schema),
}

/// Initialize logging.
///
/// This will prefer the `$RUST_LOG` environment variable, with the `-v` and
/// `-q` flags being used to modify the default log level.
///
/// For example, running `RUST_LOG=wasmer_registry=debug wasmer-deploy -q` will
/// log everything at the `error` level (`-q` means to be one level more quiet
/// than the default `warn`), but anything from the `wasmer_registry` crate will
/// be logged at the `debug` level.
fn initialize_logging(default_level: tracing::log::LevelFilter) {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_writer(std::io::stderr)
        .compact();

    let default_level = match default_level {
        tracing::log::LevelFilter::Off => tracing::level_filters::LevelFilter::OFF,
        tracing::log::LevelFilter::Error => tracing::level_filters::LevelFilter::ERROR,
        tracing::log::LevelFilter::Warn => tracing::level_filters::LevelFilter::WARN,
        tracing::log::LevelFilter::Info => tracing::level_filters::LevelFilter::INFO,
        tracing::log::LevelFilter::Debug => tracing::level_filters::LevelFilter::DEBUG,
        tracing::log::LevelFilter::Trace => tracing::level_filters::LevelFilter::TRACE,
    };

    let filter_layer = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

fn project_root() -> &'static Path {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    assert!(root.join(".git").is_dir());
    root
}
