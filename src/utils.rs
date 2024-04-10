use std::ffi::OsString;
use std::ops::Deref;
use std::path::PathBuf;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use miette::{bail, IntoDiagnostic};
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use tokio_stream::wrappers::ReadDirStream;
use tokio_stream::StreamExt;

use crate::dirs;
use crate::dirs::Dirs;
use crate::OutputFormat;

// Config

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub trait Config: Sized + DeserializeOwned + Serialize {
    // Required
    fn name(&self) -> &ConfigName;
    fn parent_dir_name() -> &'static str;
    fn toml_file_name() -> &'static str;

    // Default Implementations
    fn dir_path_of(dirs: &Dirs, name: &ConfigName) -> PathBuf {
        dirs.root_dir
            .join(Self::parent_dir_name())
            .join(name.normalized())
    }

    fn dir_path(&self, dirs: &Dirs) -> PathBuf {
        Self::dir_path_of(dirs, &self.name())
    }

    fn file_path_of(dirs: &Dirs, name: &ConfigName) -> PathBuf {
        Self::dir_path_of(dirs, name).join(Self::toml_file_name())
    }

    fn file_path(&self, dirs: &Dirs) -> PathBuf {
        Self::file_path_of(dirs, &self.name())
    }

    async fn find_match(dirs: &Dirs, name: &ConfigName) -> miette::Result<Option<ConfigName>> {
        let conflicting = Self::load(dirs, name).await?;
        let cfg_name = conflicting.map(|cfg| cfg.name().clone());
        Ok(cfg_name)
    }

    async fn get_existing(dirs: &Dirs) -> miette::Result<Vec<Self>> {
        let parent_dir_path = dirs.root_dir.join(Self::parent_dir_name());
        if !parent_dir_path.exists() {
            return Ok(vec![]);
        }

        let read_dir = tokio::fs::read_dir(parent_dir_path)
            .await
            .into_diagnostic()?;
        let read_dir = ReadDirStream::new(read_dir);
        let cfgs: Vec<Self> = read_dir
            .then(|dir| async move {
                let name = dir
                    .into_diagnostic()?
                    .file_name()
                    .into_string()
                    .map_err(os_str_to_report)?;
                Ok(Self::load(dirs, &ConfigName { raw: name }).await?.unwrap())
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<miette::Result<Vec<_>>>()?;
        Ok(cfgs)
    }

    // Result represents if there was an error reading the config
    // Option represents if a config with that name exists
    async fn load(dirs: &Dirs, name: &ConfigName) -> miette::Result<Option<Self>> {
        let path = Self::file_path_of(dirs, name);
        dirs::read_toml(&path).await
    }
    async fn save(&self, dirs: &Dirs, overwrite_existing: bool) -> miette::Result<()> {
        let conflicting_name = Self::find_match(dirs, self.name()).await?;
        match (conflicting_name, overwrite_existing) {
            (Some(name), false) => {
                bail!(
                    r#"Config with conflicting name "{}" already exists. Both normalize to "{}"."#,
                    name.raw,
                    name.normalized()
                )
            }
            _ => {
                let path = Self::file_path_of(dirs, &self.name());
                dirs::write_toml(&path, self).await
            }
        }
    }

    fn normalize_name(name: &str) -> String {
        slug::slugify(name)
    }
}

fn os_str_to_report(os_str: OsString) -> miette::Report {
    miette::Report::msg(format!("Could not convert OsStr to String: {os_str:?}"))
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ConfigName {
    pub raw: String,
}
impl ConfigName {
    pub fn new(raw_name: String) -> miette::Result<Self> {
        if raw_name.is_empty() {
            bail!("Config name cannot be an empty string.")
        }
        Ok(ConfigName { raw: raw_name })
    }

    pub fn normalized(&self) -> String {
        slug::slugify(&self.raw)
    }
}
impl Deref for ConfigName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

// Output formats

// TODO: Move OutputFormat here
pub trait OutputFormatter {
    fn to_table(&self);
    fn to_json(&self);

    fn output(&self, format: &OutputFormat) {
        match format {
            OutputFormat::Table => self.to_table(),
            OutputFormat::Json => self.to_json(),
        }
    }
}

// Dates

pub const DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S %:z";

pub fn pretty_print_date(date: &DateTime<Local>) -> String {
    date.format(DATE_FORMAT).to_string()
}

// Parsing

pub fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.split(":").collect();
    if parts.len() != 2 {
        Err(format!(
            r#"Invalid key/value pair. key/value pairs must be in the form `KEY:VALUE`.
                You submitted "{s}""#
        )
        .to_owned())
    } else if parts.iter().any(|part| part.len() == 0) {
        Err(format!(
            r#"Invalid key/value pair. The key or value was an empty string. Key/value pairs must be in the form `KEY:VALUE`.
            You submitted "{s}""#).to_string()
        )
    } else {
        Ok((parts[0].to_owned(), parts[1].to_owned()))
    }
}
