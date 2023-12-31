pub mod config;
pub mod experiment;
pub mod registry;
pub mod render;

use directories::ProjectDirs;
use once_cell::sync::Lazy;

pub static DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("io", "wasmer", "borealis").unwrap());
