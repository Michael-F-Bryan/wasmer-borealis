mod new;
mod run;
mod report;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub use crate::{new::New, run::Run, report::Report};

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());
