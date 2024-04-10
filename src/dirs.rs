use miette::{bail, IntoDiagnostic};
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_PATH_NAME: &str = "cshell";

pub struct Dirs {
    pub root_dir: PathBuf,
}

impl Dirs {
    pub fn try_new(root_dir: Option<&Path>) -> miette::Result<Self> {
        let root_dir = ensure_root_dir(root_dir)?;

        Ok(Self { root_dir })
    }
}

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

pub const WALLETS_PARENT_DIR: &str = "wallets";
pub const WALLET_CONFIG_FILENAME: &str = "config.toml";

pub const U5C_PARENT_DIR: &str = "utxorpc";
pub const U5C_CONFIG_FILE_NAME: &str = "config.toml";

pub async fn read_toml<'de, T>(path: &Path) -> miette::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let has_toml_ext = path.extension() == Some("toml".as_ref());
    if path.is_file() && has_toml_ext {
        let contents: Vec<u8> = tokio::fs::read(path).await.into_diagnostic()?;
        let contents: String = String::from_utf8(contents).into_diagnostic()?;

        let t = toml::from_str::<T>(&contents).into_diagnostic()?;
        Ok(Some(t))
    } else {
        Ok(None)
    }
}

pub async fn write_toml<T>(path: &Path, t: &T) -> miette::Result<()>
where
    T: Serialize,
{
    let contents = toml::to_string(t).into_diagnostic()?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.into_diagnostic()?;
    }
    tokio::fs::write(path, contents).await.into_diagnostic()
}
