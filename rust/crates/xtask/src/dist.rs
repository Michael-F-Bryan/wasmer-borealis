use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Error};
use once_cell::sync::Lazy;
use xshell::Shell;
use zip::{write::FileOptions, ZipWriter};

const PROJECT_NAME: &str = "wasmer-borealis";
const BINARIES: &[&str] = &["wasmer-borealis"];
const STATIC_FILES: &[StaticFile] = &[
    StaticFile::new("README.md"),
    StaticFile::new("LICENSE_MIT.md"),
    StaticFile::new("LICENSE_APACHE.md"),
];

#[derive(clap::Parser, Debug)]
pub struct Dist {
    /// The drectory to save release artifacts to.
    #[clap(long, default_value = DIST_DIR.as_os_str())]
    output_dir: PathBuf,
}

impl Dist {
    pub(crate) fn run(self) -> Result<(), Error> {
        let sh = Shell::new()?;
        sh.change_dir(crate::project_root());

        sh.remove_path(&self.output_dir)?;
        sh.create_dir(&self.output_dir)?;

        self.binary(&sh)?;
        self.files(&sh)?;
        self.bundle(&sh)?;

        Ok(())
    }

    fn bundle(&self, sh: &Shell) -> Result<(), Error> {
        let parent_dir = self.output_dir.parent().unwrap();
        let triple = host_triple(sh)?;
        let dest = parent_dir.join(format!("{PROJECT_NAME}.{triple}.zip"));

        let f = File::create(&dest)
            .with_context(|| format!("Unable to create \"{}\"", dest.display()))?;

        let mut writer = ZipWriter::new(BufWriter::new(f));
        self.bundle_dir(&self.output_dir, &mut writer)?;
        writer.finish()?.flush()?;

        Ok(())
    }

    fn bundle_dir(&self, dir: &Path, writer: &mut ZipWriter<BufWriter<File>>) -> Result<(), Error> {
        for entry in dir.read_dir()? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            let stripped = path
                .strip_prefix(&self.output_dir)
                .unwrap()
                .display()
                .to_string()
                .replace('\\', "/");

            if file_type.is_file() {
                writer.start_file(stripped, FileOptions::default())?;
                let f = File::open(&path)?;
                let mut reader = BufReader::new(f);
                std::io::copy(&mut reader, writer)?;
            } else if file_type.is_dir() {
                writer.add_directory(stripped, FileOptions::default())?;
                self.bundle_dir(&path, writer)?;
            }
        }

        Ok(())
    }

    /// Compile the executables and strip them to reduce size.
    fn binary(&self, sh: &Shell) -> Result<(), Error> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        sh.cmd(cargo)
            .args(["build", "--release", "--workspace", "--locked"])
            .run()?;

        let target_dir = crate::project_root().join("target").join("release");

        let can_strip = sh
            .cmd("strip")
            .arg("--version")
            .ignore_stderr()
            .ignore_stdout()
            .quiet()
            .run()
            .is_ok();

        for binary in BINARIES {
            let mut src = target_dir.join(binary);
            let bin_dir = self.output_dir.join("bin");
            let mut dest = bin_dir.join(binary);

            if cfg!(windows) {
                src.set_extension("exe");
                dest.set_extension("exe");
            }

            sh.create_dir(&bin_dir)?;
            sh.copy_file(&src, &dest)?;

            if can_strip {
                sh.cmd("strip").arg(&dest).run()?;
            }
        }

        Ok(())
    }

    fn files(&self, sh: &Shell) -> Result<(), Error> {
        for StaticFile { src, dest } in STATIC_FILES {
            let dest = self.output_dir.join(dest);
            if let Some(parent) = dest.parent() {
                sh.create_dir(parent)?;
            }

            sh.copy_file(src, dest)?;
        }

        Ok(())
    }
}

static DIST_DIR: Lazy<PathBuf> = Lazy::new(|| crate::project_root().join("target/dist"));

fn host_triple(sh: &Shell) -> Result<String, Error> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let stdout = sh.cmd(rustc).arg("--version").arg("--verbose").read()?;

    for line in stdout.lines() {
        if let Some(triple) = line.strip_prefix("host: ") {
            return Ok(triple.trim().to_string());
        }
    }

    Err(Error::msg("Unable to determine the host target triple"))
}

struct StaticFile {
    src: &'static str,
    dest: &'static str,
}

impl StaticFile {
    const fn new(filename: &'static str) -> Self {
        StaticFile {
            src: filename,
            dest: filename,
        }
    }
}
