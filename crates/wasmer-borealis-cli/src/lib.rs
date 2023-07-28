mod new;
mod run;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub use crate::{new::New, run::Run};

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());
