mod new;
mod report;
mod run;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub use crate::{new::New, report::Report, run::Run};

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());

pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
