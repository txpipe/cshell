use miette::{bail, IntoDiagnostic};
use std::path::{Path, PathBuf};

const DEFAULT_PATH_NAME: &str = "cshell";

fn default_root_dir() -> miette::Result<PathBuf> {
    if let Some(path) = directories::ProjectDirs::from("", "", DEFAULT_PATH_NAME) {
        return Ok(path.data_dir().into());
    }

    bail!("Could not automatically determine path to c-shell data directory.\nUse root_dir parameter or CSHELL_ROOT_DIR environment variable.");
}

pub fn ensure_root_dir(explicit: Option<&Path>) -> miette::Result<PathBuf> {
    let defined = explicit
        .map(|p| p.join(DEFAULT_PATH_NAME))
        .unwrap_or(default_root_dir()?);

    std::fs::create_dir_all(&defined).into_diagnostic()?;

    Ok(defined)
}

pub struct Dirs {
    pub root_dir: PathBuf,
}

impl Dirs {
    pub fn try_new(root_dir: Option<&Path>) -> miette::Result<Self> {
        let root_dir = ensure_root_dir(root_dir)?;

        Ok(Self { root_dir })
    }
}
