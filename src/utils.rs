use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde::{de, Deserialize, Deserializer, Serializer};

pub const DATE_FORMAT: &str = "%Y-%m-%d";

pub fn serialize_date<S>(date: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let date_str = date.format(DATE_FORMAT).to_string();
    serializer.serialize_str(&date_str)
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    let date_str: String = Deserialize::deserialize(deserializer)?;

    let parsed_date =
        NaiveDate::parse_from_str(&date_str, DATE_FORMAT).map_err(de::Error::custom)?;

    let local_date = Local
        .from_local_datetime(&NaiveDateTime::new(parsed_date, NaiveTime::default()))
        .unwrap();

    Ok(local_date)
}

pub trait OutputFormatter {
    fn to_table(&self);
    fn to_json(&self);
}
