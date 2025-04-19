use bech32::{FromBase32, ToBase32};
use bip39::rand_core::{CryptoRng, RngCore};
use bip39::{Language, Mnemonic};
use chrono::{DateTime, Local};
use comfy_table::Table;
use cryptoxide::chacha20poly1305::ChaCha20Poly1305;
use cryptoxide::kdf::argon2;
use cryptoxide::{hmac::Hmac, pbkdf2::pbkdf2, sha2::Sha512};
use ed25519_bip32::{self, XPrv, XPub, XPRV_SIZE};
use miette::{Context, IntoDiagnostic};
use pallas::{
    crypto::key::ed25519::{self, PublicKey, SecretKey, SecretKeyExtended, Signature},
    ledger::{
        addresses::{Address, Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart},
        traverse::ComputeHash,
    },
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;

use crate::{output::OutputFormatter, utils, utils::Name};

const ITERATIONS: u32 = 2500;
const VERSION_SIZE: usize = 1;
const SALT_SIZE: usize = 16;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

pub type NewWallet = (String, Wallet);

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Wallet {
    pub name: Name,
    #[serde(with = "hex::serde")]
    pub public_key: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "utils::option_hex_vec_u8")]
    pub encrypted_private_key: Option<Vec<u8>>,
    pub created: DateTime<Local>,
    pub modified: DateTime<Local>,
    pub is_default: bool,
}

impl Wallet {
    pub fn try_from(name: &str, password: &str, is_default: bool) -> miette::Result<NewWallet> {
        let (private_key, mnemonic) =
            Bip32PrivateKey::generate_with_mnemonic(OsRng, password.to_string());
        let public_key = private_key.to_public().as_bytes();

        let encrypted_private_key = encrypt_private_key(
            OsRng,
            private_key.to_ed25519_private_key(),
            &password.to_string(),
        );

        Ok((
            mnemonic.to_string(),
            Self {
                name: Name::try_from(name)?,
                encrypted_private_key: Some(encrypted_private_key),
                public_key,
                created: Local::now(),
                modified: Local::now(),
                is_default,
            },
        ))
    }

    pub fn try_from_mnemonic(
        name: &str,
        password: &str,
        mnemonic: &str,
        is_default: bool,
    ) -> miette::Result<Self> {
        let private_key =
            Bip32PrivateKey::from_bip39_mnenomic(mnemonic.to_string(), password.to_string())?;
        let public_key = private_key.to_public().as_bytes();

        let encrypted_private_key = encrypt_private_key(
            OsRng,
            private_key.to_ed25519_private_key(),
            &password.to_string(),
        );

        Ok(Self {
            name: Name::try_from(name)?,
            encrypted_private_key: Some(encrypted_private_key),
            public_key,
            created: Local::now(),
            modified: Local::now(),
            is_default,
        })
    }

    pub fn address(&self, is_testnet: bool) -> Address {
        let pk = match self.encrypted_private_key {
            Some(_) => Bip32PublicKey::from_bytes(self.public_key.clone().try_into().unwrap())
                .to_ed25519_pubkey(),
            None => PublicKey::from_str(&hex::encode(&self.public_key)).unwrap(),
        };

        if is_testnet {
            ShelleyAddress::new(
                Network::Testnet,
                ShelleyPaymentPart::key_hash(pk.compute_hash()),
                ShelleyDelegationPart::Null,
            )
            .into()
        } else {
            ShelleyAddress::new(
                Network::Mainnet,
                ShelleyPaymentPart::key_hash(pk.compute_hash()),
                ShelleyDelegationPart::Null,
            )
            .into()
        }
    }
}

impl OutputFormatter for Wallet {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Property", "Value"]);

        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec!["Public Key Hash", &hex::encode(&self.public_key)]);
        table.add_row(vec!["Address (mainnet)", &self.address(false).to_string()]);
        table.add_row(vec!["Address (testnet)", &self.address(true).to_string()]);
        table.add_row(vec!["Created", &utils::pretty_print_date(&self.created)]);
        table.add_row(vec!["Modified", &utils::pretty_print_date(&self.modified)]);

        println!("{table}");
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "name": &self.name,
                "public_key": hex::encode(&self.public_key),
                "addresses": {
                    "mainnet": &self.address(false).to_string(),
                    "testnet": &self.address(true).to_string(),
                },
                "created": self.created,
                "modified": self.modified,
                "is_default": self.is_default,
            }))
            .unwrap()
        );
    }
}

impl OutputFormatter for &Vec<Wallet> {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "Created", "Modified", "Is Default?"]);

        for wallet in self.iter() {
            table.add_row(vec![
                wallet.name.to_string(),
                utils::pretty_print_date(&wallet.created),
                utils::pretty_print_date(&wallet.modified),
                wallet.is_default.to_string(),
            ]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &self
                    .iter()
                    .map(|wallet| {
                        json!({
                            "name": &wallet.name,
                            "public_key": hex::encode(&wallet.public_key),
                            "addresses": {
                                "mainnet": &wallet.address(false).to_string(),
                                "testnet": &wallet.address(true).to_string(),
                            },
                            "created": wallet.created,
                            "modified": wallet.modified,
                            "is_default": wallet.is_default,
                        })
                    })
                    .collect::<Vec<Value>>(),
            )
            .unwrap()
        );
    }
}

impl OutputFormatter for NewWallet {
    fn to_table(&self) {
        println!("Your mnemonic phrase is the following:");
        println!("\n");
        println!("* {}", self.0);
        println!("\n");
        println!("Save this phrase somewhere safe to restore your wallet if it ever gets lost.");

        self.1.to_table();
    }

    fn to_json(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "name": &self.1.name,
                "mnemonic": &self.0,
                "public_key": hex::encode(&self.1.public_key),
                "addresses": {
                    "mainnet": &self.1.address(false).to_string(),
                    "testnet": &self.1.address(true).to_string(),
                },
                "created": self.1.created,
                "modified": self.1.modified,
                "is_default": self.1.is_default,
            }))
            .unwrap()
        );
    }
}

/// A standard or extended Ed25519 secret key
pub enum PrivateKey {
    Normal(SecretKey),
    Extended(SecretKeyExtended),
}

impl PrivateKey {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Self::Normal(_) => SecretKey::SIZE,
            Self::Extended(_) => SecretKeyExtended::SIZE,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        match self {
            Self::Normal(x) => x.public_key(),
            Self::Extended(x) => x.public_key(),
        }
    }

    pub fn sign<T>(&self, msg: T) -> Signature
    where
        T: AsRef<[u8]>,
    {
        match self {
            Self::Normal(x) => x.sign(msg),
            Self::Extended(x) => x.sign(msg),
        }
    }

    pub(crate) fn as_bytes(&self) -> Vec<u8> {
        match self {
            Self::Normal(x) => {
                let bytes: [u8; SecretKey::SIZE] = unsafe { SecretKey::leak_into_bytes(x.clone()) };
                bytes.to_vec()
            }
            Self::Extended(x) => {
                let bytes: [u8; SecretKeyExtended::SIZE] =
                    unsafe { SecretKeyExtended::leak_into_bytes(x.clone()) };
                bytes.to_vec()
            }
        }
    }
}

impl From<SecretKey> for PrivateKey {
    fn from(key: SecretKey) -> Self {
        PrivateKey::Normal(key)
    }
}

impl From<SecretKeyExtended> for PrivateKey {
    fn from(key: SecretKeyExtended) -> Self {
        PrivateKey::Extended(key)
    }
}

/// Ed25519-BIP32 HD Private Key
#[derive(Debug, PartialEq, Eq)]
pub struct Bip32PrivateKey(ed25519_bip32::XPrv);
impl Bip32PrivateKey {
    const BECH32_HRP: &'static str = "xprv";

    pub fn generate<T: RngCore + CryptoRng>(mut rng: T) -> Self {
        let mut buf = [0u8; XPRV_SIZE];
        rng.fill_bytes(&mut buf);
        let xprv = XPrv::normalize_bytes_force3rd(buf);

        Self(xprv)
    }

    pub fn generate_with_mnemonic<T: RngCore + CryptoRng>(
        mut rng: T,
        password: String,
    ) -> (Self, Mnemonic) {
        let mut buf = [0u8; 64];
        rng.fill_bytes(&mut buf);

        let bip39 = Mnemonic::generate_in_with(&mut rng, Language::English, 24).unwrap();

        let entropy = bip39.clone().to_entropy();

        let mut pbkdf2_result = [0; XPRV_SIZE];

        const ITER: u32 = 4096; // TODO: BIP39 says 2048, CML uses 4096?

        let mut mac = Hmac::new(Sha512::new(), password.as_bytes());
        pbkdf2(&mut mac, &entropy, ITER, &mut pbkdf2_result);

        (Self(XPrv::normalize_bytes_force3rd(pbkdf2_result)), bip39)
    }

    pub fn from_bytes(bytes: [u8; 96]) -> miette::Result<Self> {
        XPrv::from_bytes_verified(bytes)
            .map(Self)
            .into_diagnostic()
            .context("xprv error")
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.0.as_ref().to_vec()
    }

    pub fn from_bip39_mnenomic(mnemonic: String, password: String) -> miette::Result<Self> {
        let bip39 = Mnemonic::parse(mnemonic)
            .into_diagnostic()
            .context("Error parsing mnemonic")?;
        let entropy = bip39.to_entropy();

        let mut pbkdf2_result = [0; XPRV_SIZE];

        const ITER: u32 = 4096; // TODO: BIP39 says 2048, CML uses 4096?

        let mut mac = Hmac::new(Sha512::new(), password.as_bytes());
        pbkdf2(&mut mac, &entropy, ITER, &mut pbkdf2_result);

        Ok(Self(XPrv::normalize_bytes_force3rd(pbkdf2_result)))
    }

    pub fn derive(&self, index: u32) -> Self {
        Self(self.0.derive(ed25519_bip32::DerivationScheme::V2, index))
    }

    pub fn to_ed25519_private_key(&self) -> PrivateKey {
        PrivateKey::Extended(unsafe {
            // The use of unsafe is allowed here. The key is an Extended Secret Key
            // already because it passed through the ed25519_bip32 crates checks
            SecretKeyExtended::from_bytes_unchecked(self.0.extended_secret_key())
        })
    }

    pub fn to_public(&self) -> Bip32PublicKey {
        Bip32PublicKey(self.0.public())
    }

    pub fn chain_code(&self) -> [u8; 32] {
        *self.0.chain_code()
    }

    pub fn to_bech32(&self) -> String {
        bech32::encode(
            Self::BECH32_HRP,
            self.as_bytes().to_base32(),
            bech32::Variant::Bech32,
        )
        .unwrap()
    }

    pub fn from_bech32(bech32: String) -> miette::Result<Self> {
        let (hrp, data, _) = bech32::decode(&bech32)
            .into_diagnostic()
            .context("Invalid bech32")?;
        if hrp != Self::BECH32_HRP {
            miette::bail!("Invalid bech32")
        } else {
            let data = Vec::<u8>::from_base32(&data)
                .into_diagnostic()
                .context("Invalid bech32")?;
            match data.try_into() {
                Ok(bytes) => Self::from_bytes(bytes),
                Err(_) => miette::bail!("Unexpected Bech32 length"),
            }
        }
    }
}

/// Ed25519-BIP32 HD Public Key
#[derive(Debug, PartialEq, Eq)]
pub struct Bip32PublicKey(ed25519_bip32::XPub);

impl Bip32PublicKey {
    const BECH32_HRP: &'static str = "xpub";

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(XPub::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.0.as_ref().to_vec()
    }

    pub fn derive(&self, index: u32) -> miette::Result<Self> {
        self.0
            .derive(ed25519_bip32::DerivationScheme::V2, index)
            .map(Self)
            .into_diagnostic()
            .context("Failed to derive key")
    }

    pub fn to_ed25519_pubkey(&self) -> ed25519::PublicKey {
        self.0.public_key().into()
    }

    pub fn chain_code(&self) -> [u8; 32] {
        *self.0.chain_code()
    }

    pub fn to_bech32(&self) -> String {
        bech32::encode(
            Self::BECH32_HRP,
            self.as_bytes().to_base32(),
            bech32::Variant::Bech32,
        )
        .unwrap()
    }

    pub fn from_bech32(bech32: String) -> miette::Result<Self> {
        let (hrp, data, _) = bech32::decode(&bech32)
            .into_diagnostic()
            .context("Invalid Bech32")?;
        if hrp != Self::BECH32_HRP {
            Err(miette::diagnostic!("Invalid Bech32").into())
        } else {
            let data = Vec::<u8>::from_base32(&data)
                .into_diagnostic()
                .context("Invalid Bech32")?;
            match data.try_into() {
                Ok(bytes) => Ok(Self::from_bytes(bytes)),
                Err(_) => miette::bail!("Unexpected Bech32 length"),
            }
        }
    }
}

pub fn encrypt_private_key<Rng>(mut rng: Rng, private_key: PrivateKey, password: &String) -> Vec<u8>
where
    Rng: RngCore + CryptoRng,
{
    let salt = {
        let mut salt = [0u8; SALT_SIZE];
        rng.fill_bytes(&mut salt);
        salt
    };

    let sym_key: [u8; 32] = argon2::argon2(
        &argon2::Params::argon2d().iterations(ITERATIONS).unwrap(),
        password.as_bytes(),
        &salt,
        &[],
        &[],
    );

    let nonce = {
        let mut nonce = [0u8; NONCE_SIZE];
        rng.fill_bytes(&mut nonce);
        nonce
    };

    let mut chacha20 = ChaCha20Poly1305::new(&sym_key, &nonce, &[]);

    let data_size = private_key.len();

    let (ciphertext, ct_tag) = {
        let mut ciphertext = vec![0u8; data_size];
        let mut ct_tag = [0u8; 16];
        chacha20.encrypt(&private_key.as_bytes(), &mut ciphertext, &mut ct_tag);

        (ciphertext, ct_tag)
    };

    // (version || salt || nonce || tag || ciphertext)
    let mut out = Vec::with_capacity(VERSION_SIZE + SALT_SIZE + NONCE_SIZE + TAG_SIZE + data_size);

    out.push(1);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct_tag);
    out.extend_from_slice(&ciphertext);

    out
}

#[allow(unused)]
pub fn decrypt_private_key(password: &String, data: Vec<u8>) -> miette::Result<PrivateKey> {
    let data_len_without_ct = VERSION_SIZE + SALT_SIZE + NONCE_SIZE + TAG_SIZE;

    let ciphertext_len = if data.len() == (data_len_without_ct + SecretKey::SIZE) {
        SecretKey::SIZE
    } else if data.len() == (data_len_without_ct + SecretKeyExtended::SIZE) {
        SecretKeyExtended::SIZE
    } else {
        miette::bail!("Invalid wrapper size")
    };

    let mut cursor = 0;

    let _version = &data[cursor];
    cursor += VERSION_SIZE;

    let salt = &data[cursor..cursor + SALT_SIZE];
    cursor += SALT_SIZE;

    let nonce = &data[cursor..cursor + NONCE_SIZE];
    cursor += NONCE_SIZE;

    let tag = &data[cursor..cursor + TAG_SIZE];
    cursor += TAG_SIZE;

    let ciphertext = &data[cursor..cursor + ciphertext_len];

    let sym_key: [u8; 32] = argon2::argon2(
        &argon2::Params::argon2d().iterations(ITERATIONS).unwrap(),
        password.as_bytes(),
        salt,
        &[],
        &[],
    );

    let mut chacha20 = ChaCha20Poly1305::new(&sym_key, nonce, &[]);

    match ciphertext_len {
        SecretKey::SIZE => {
            let mut plaintext = [0u8; SecretKey::SIZE];

            if chacha20.decrypt(ciphertext, &mut plaintext, tag) {
                let secret_key: SecretKey = plaintext.into();

                Ok(secret_key.into())
            } else {
                Err(miette::bail!("Wrapper data failed to decrypt"))
            }
        }
        SecretKeyExtended::SIZE => {
            let mut plaintext = [0u8; SecretKeyExtended::SIZE];

            if chacha20.decrypt(ciphertext, &mut plaintext, tag) {
                let secret_key = SecretKeyExtended::from_bytes(plaintext)
                    .into_diagnostic()
                    .context("decoding secret key")?;

                Ok(secret_key.into())
            } else {
                Err(miette::bail!("Wrapper data failed to decrypt"))
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decrypt_private_key, encrypt_private_key, Bip32PrivateKey, Bip32PublicKey, PrivateKey,
    };
    use bip39::rand_core::OsRng;
    use pallas::crypto::key::ed25519::{SecretKey, SecretKeyExtended};

    #[test]
    fn mnemonic_roundtrip() {
        let (xprv, mne) = Bip32PrivateKey::generate_with_mnemonic(OsRng, "".into());

        let xprv_from_mne =
            Bip32PrivateKey::from_bip39_mnenomic(mne.to_string(), "".into()).unwrap();

        assert_eq!(xprv, xprv_from_mne)
    }

    #[test]
    fn bech32_roundtrip() {
        let xprv = Bip32PrivateKey::generate(OsRng);

        let xprv_bech32 = xprv.to_bech32();

        let decoded_xprv = Bip32PrivateKey::from_bech32(xprv_bech32).unwrap();

        assert_eq!(xprv, decoded_xprv);

        let xpub = xprv.to_public();

        let xpub_bech32 = xpub.to_bech32();

        let decoded_xpub = Bip32PublicKey::from_bech32(xpub_bech32).unwrap();

        assert_eq!(xpub, decoded_xpub)
    }

    #[test]
    fn private_key_encryption_roundtrip() {
        let password = "hunter123";

        // --- standard

        let private_key = PrivateKey::Normal(SecretKey::new(OsRng));

        let private_key_bytes = private_key.as_bytes();

        let encrypted_priv_key = encrypt_private_key(OsRng, private_key, &password.into());

        let decrypted_privkey = decrypt_private_key(&password.into(), encrypted_priv_key).unwrap();

        assert_eq!(private_key_bytes, decrypted_privkey.as_bytes());

        // --- extended

        let private_key = PrivateKey::Extended(SecretKeyExtended::new(OsRng));

        let private_key_bytes = private_key.as_bytes();

        let encrypted_priv_key = encrypt_private_key(OsRng, private_key, &password.into());

        let decrypted_privkey = decrypt_private_key(&password.into(), encrypted_priv_key).unwrap();

        assert_eq!(private_key_bytes, decrypted_privkey.as_bytes())
    }
}
