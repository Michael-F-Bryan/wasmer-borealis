pub mod experiment;
mod new;
mod run;
mod queries;

use anyhow::Error;
use clap::Parser;
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use tracing_subscriber::EnvFilter;

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());

fn main() -> Result<(), Error> {
    let Args { verbosity, cmd } = Args::parse();

    initialize_logging(verbosity.log_level_filter());

    match cmd {
        Cmd::Run(r) => r.execute(),
        Cmd::New(n) => n.execute(),
    }
}

#[derive(Debug, Parser)]
#[clap(about, version, author)]
struct Args {
    #[clap(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
enum Cmd {
    /// Create a new experiment.
    New(new::New),
    /// Run an experiment.
    Run(run::Run),
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
    let default_level = match default_level {
        tracing::log::LevelFilter::Off => tracing::level_filters::LevelFilter::OFF,
        tracing::log::LevelFilter::Error => tracing::level_filters::LevelFilter::ERROR,
        tracing::log::LevelFilter::Warn => tracing::level_filters::LevelFilter::WARN,
        tracing::log::LevelFilter::Info => tracing::level_filters::LevelFilter::INFO,
        tracing::log::LevelFilter::Debug => tracing::level_filters::LevelFilter::DEBUG,
        tracing::log::LevelFilter::Trace => tracing::level_filters::LevelFilter::TRACE,
    };

    let env = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_writer(std::io::stderr)
        .with_env_filter(env)
        .init();
}
