use crate::utils::ConfigName;
use crate::utils::OutputFormatter;
use crate::{dirs, utils};
use chrono::{DateTime, Local};
use comfy_table::Table;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Utxorpc {
    pub version: String,
    pub name: ConfigName,
    pub network: String,
    pub is_testnet: bool,
    pub url: Url,
    pub headers: Vec<(String, String)>,

    pub created_on: DateTime<Local>,
    pub last_updated: DateTime<Local>,
}
impl Utxorpc {
    pub fn new(
        name: String,
        url: Url,
        network: String,
        is_test_net: bool,
        headers: Vec<(String, String)>,
    ) -> miette::Result<Self> {
        let now = Local::now();
        Ok(Self {
            version: crate::utils::VERSION.to_owned(),
            name: ConfigName::new(name)?,
            network,
            is_testnet: is_test_net,
            url,
            headers,
            created_on: now,
            last_updated: now,
        })
    }

    pub fn update(
        &mut self,
        url: Option<Url>,
        headers: Option<Vec<(String, String)>>,
        is_testnet: Option<bool>,
    ) {
        if let Some(url) = url {
            self.url = url;
        }
        if let Some(headers) = headers {
            self.headers = headers;
        }
        if let Some(is_testnet) = is_testnet {
            self.is_testnet = is_testnet
        };
        self.last_updated = Local::now();
    }
}

impl crate::utils::Config for Utxorpc {
    fn name(&self) -> &ConfigName {
        &self.name
    }

    fn config_type() -> &'static str {
        "Utxorpc"
    }

    fn parent_dir_name() -> &'static str {
        &dirs::U5C_PARENT_DIR
    }

    fn toml_file_name() -> &'static str {
        &dirs::U5C_CONFIG_FILE_NAME
    }
}

impl OutputFormatter for Utxorpc {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Property", "Value"]);

        table.add_row(vec!["Name", &self.name.raw]);
        table.add_row(vec!["URL", &self.url.to_string()]);
        for (header, value) in &self.headers {
            table.add_row(vec!["Header", header, value]);
        }
        table.add_row(vec!["Network", &self.network]);
        table.add_row(vec!["Is testnet", &self.is_testnet.to_string()]);
        table.add_row(vec![
            "Created on",
            &utils::pretty_print_date(&self.created_on),
        ]);
        table.add_row(vec![
            "Last updated",
            &utils::pretty_print_date(&self.last_updated),
        ]);

        println!("{table}");
    }

    fn to_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}

impl OutputFormatter for Vec<Utxorpc> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "URL"]);

        for utxorpc in self {
            table.add_row(vec![&utxorpc.name.raw, &utxorpc.url.to_string()]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let json: String = serde_json::to_string_pretty(self).unwrap();
        println!("{json}");
    }
}
