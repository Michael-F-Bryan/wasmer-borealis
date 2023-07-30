mod new;
mod run;
mod run2;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub use crate::{new::New, run::Run, run2::Run as Run2};

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());
