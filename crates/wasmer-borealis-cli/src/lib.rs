mod new;
mod report;
mod run;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub use crate::{new::New, report::Report, run::Run};

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());
