use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{
    provider::types::Provider,
    utils::{read_toml, write_toml},
    wallet::types::Wallet,
};

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
    inner: StoreInner,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Clone)]
pub struct StoreInner {
    pub wallets: Vec<Wallet>,
    pub providers: Vec<Provider>,
}

impl Store {
    pub fn open(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let path = path.unwrap_or({
            // Get the home directory.  This is platform-dependent.
            let home_dir = match std::env::var("HOME") {
                Ok(path) => PathBuf::from(path),
                Err(_) => match std::env::var("USERPROFILE") {
                    Ok(path) => PathBuf::from(path),
                    Err(_) => {
                        bail!("Could not determine home directory");
                    }
                },
            };

            // Create the full path to the file.
            home_dir.join("cshell.toml")
        });
        let inner = read_toml(&path)?.unwrap_or_default();
        Ok(Self { path, inner })
    }

    pub fn write(&self) -> anyhow::Result<()> {
        write_toml(&self.path, &self.inner)
    }

    pub fn default_wallet(&self) -> Option<&Wallet> {
        self.inner.wallets.iter().find(|wallet| wallet.is_default)
    }

    pub fn add_wallet(&mut self, wallet: &Wallet) -> anyhow::Result<()> {
        self.inner.wallets.push(wallet.clone());
        self.write()
    }

    pub fn remove_wallet(&mut self, wallet: Wallet) -> anyhow::Result<()> {
        match self.inner.wallets.iter().position(|x| *x == wallet) {
            Some(idx) => {
                self.inner.wallets.remove(idx);
                self.write()
            }
            None => bail!("Wallet not on store."),
        }
    }

    pub fn find_wallet(&self, name: &str) -> Option<&Wallet> {
        self.inner
            .wallets
            .iter()
            .find(|w| w.name.to_string() == name)
    }

    pub fn wallets(&self) -> &Vec<Wallet> {
        &self.inner.wallets
    }

    pub fn default_provider(&self) -> Option<&Provider> {
        self.inner
            .providers
            .iter()
            .find(|provider| provider.is_default())
    }

    pub fn providers(&self) -> &Vec<Provider> {
        &self.inner.providers
    }

    pub fn add_provider(&mut self, provider: &Provider) -> anyhow::Result<()> {
        self.inner.providers.push(provider.clone());
        self.write()
    }

    pub fn find_provider(&self, name: &str) -> Option<&Provider> {
        self.inner.providers.iter().find(|p| p.name() == name)
    }

    pub fn remove_provider(&mut self, provider: Provider) -> anyhow::Result<()> {
        match self.inner.providers.iter().position(|x| *x == provider) {
            Some(idx) => {
                self.inner.providers.remove(idx);
                self.write()
            }
            None => bail!("Provider not on store."),
        }
    }
}
