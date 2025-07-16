use chrono::{DateTime, Local};
use miette::{bail, IntoDiagnostic};
use num_format::{Locale, ToFormattedString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(String);
impl TryFrom<String> for Name {
    type Error = miette::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            bail!("Value cannot be empty.")
        }
        Ok(Name(value))
    }
}
impl TryFrom<&str> for Name {
    type Error = miette::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.to_string();
        if value.is_empty() {
            bail!("Value cannot be empty.")
        }
        Ok(Name(value))
    }
}

impl Name {
    pub fn normalized(&self) -> String {
        slug::slugify(&self.0)
    }
}
impl std::ops::Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait AdaFormat {
    fn format_ada(self) -> String;
}
impl AdaFormat for u64 {
    fn format_ada(self) -> String {
        let decimals = 6;
        let factor = 10u64.pow(decimals);
        let whole = self / factor;
        let fraction = self % factor;

        let whole_str = whole.to_formatted_string(&Locale::en);
        let fraction_str = format!("{:0width$}", fraction, width = decimals as usize);

        format!("{whole_str}.{fraction_str}")
    }
}

pub fn show_is_current(option: impl std::fmt::Display, is_current: bool) -> String {
    if is_current {
        format!("{option} (current)")
    } else {
        format!("{option}")
    }
}

// Dates
pub const DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S %:z";

pub fn pretty_print_date(date: &DateTime<Local>) -> String {
    date.format(DATE_FORMAT).to_string()
}

pub fn read_toml<T>(path: &std::path::Path) -> miette::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let has_toml_ext = path.extension() == Some("toml".as_ref());
    if path.is_file() && has_toml_ext {
        let contents: Vec<u8> = std::fs::read(path).into_diagnostic()?;
        let contents: String = String::from_utf8(contents).into_diagnostic()?;

        let t = toml::from_str::<T>(&contents).into_diagnostic()?;
        Ok(Some(t))
    } else {
        Ok(None)
    }
}

pub fn write_toml<T>(path: &std::path::Path, t: &T) -> miette::Result<()>
where
    T: Serialize,
{
    let contents = toml::to_string(t).into_diagnostic()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }
    std::fs::write(path, contents).into_diagnostic()
}

pub fn clip(input: impl ToString, len: usize) -> String {
    let input = input.to_string();
    if input.len() <= len {
        return input;
    }

    let ellipsis_len = 3; // Length of "..."
    if len <= ellipsis_len {
        return input.chars().take(len).collect();
    }

    let chars_to_take = len - ellipsis_len;
    let first_half_len = chars_to_take / 2;
    let second_half_len = chars_to_take - first_half_len;

    let first_part: String = input.chars().take(first_half_len).collect();
    let last_part: String = input.chars().skip(input.len() - second_half_len).collect();

    format!("{first_part}...{last_part}")
}

// Helper functions for serializing Option<Vec<u8>> as hex
pub mod option_hex_vec_u8 {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(opt_vec: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_vec {
            Some(vec) => {
                let hex_string = hex::encode(vec);
                serializer.serialize_str(&hex_string)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                let vec = hex::decode(s).map_err(D::Error::custom)?;
                Ok(Some(vec))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip() {
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("hello", 5), "hello");
        assert_eq!(clip("thisisalongstring", 10), "thi...ring");
        assert_eq!(clip("anotherlongstring", 11), "anot...ring");
        assert_eq!(clip("verylong", 3), "ver");
        assert_eq!(clip("short", 2), "sh");
        assert_eq!(clip("", 5), "");
    }
}
