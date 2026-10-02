//! `zdisclosure` for the browser, over [`zdisclosure_core`].
//!
//! The bindings take strings and byte arrays and answer with the report types below.
//! `tsify` writes the TypeScript declaration of each one into the generated glue, so the
//! page has no copy of them to keep in step. A `u64` reaches the page as a `bigint`. The
//! transaction bytes come from the page: a browser has neither files nor the sockets a
//! lightwalletd client needs, so verification runs on hex the site fetches or the user
//! pastes. Nothing here contacts a server.

use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use zcash_protocol::consensus::{self, BlockHeight, NetworkType};
use zdisclosure_core::{Error, check, input, network, produce, tx, view};

fn params_for(name: &str) -> Result<network::Params, Error> {
    match name {
        "main" => Ok(network::Params::Public(consensus::Network::MainNetwork)),
        "test" => Ok(network::Params::Public(consensus::Network::TestNetwork)),
        "regtest" => Ok(network::Params::Regtest(network::default_regtest())),
        other => Err(Error::new(format!("unknown network {other:?}"))),
    }
}

/// The network a Disclosure names.
#[derive(Debug, PartialEq, Serialize, Tsify)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    /// Mainnet.
    Main,
    /// Testnet.
    Test,
    /// A regtest chain.
    Regtest,
}

impl From<NetworkType> for Network {
    fn from(network: NetworkType) -> Self {
        match network {
            NetworkType::Main => Self::Main,
            NetworkType::Test => Self::Test,
            NetworkType::Regtest => Self::Regtest,
        }
    }
}

/// The side of the payment an output was opened from.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// An incoming viewing key decrypted the output.
    Receiver,
    /// An outgoing viewing key recovered the output.
    Sender,
}

impl From<zcash_disclosure::Role> for Role {
    fn from(role: zcash_disclosure::Role) -> Self {
        match role {
            zcash_disclosure::Role::Receiver => Self::Receiver,
            zcash_disclosure::Role::Sender => Self::Sender,
        }
    }
}

/// The key scope an output was derived at. Internal is change.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// An address given to a payer.
    External,
    /// The wallet's own change.
    Internal,
}

impl From<zip32::Scope> for Scope {
    fn from(scope: zip32::Scope) -> Self {
        match scope {
            zip32::Scope::External => Self::External,
            zip32::Scope::Internal => Self::Internal,
        }
    }
}

/// An Output Item as the Disclosure carries it, before any verification.
#[derive(Serialize, Tsify)]
pub struct ItemReport {
    /// The position of the output in the Sapling outputs.
    pub index: u32,
    /// The Item's typecode.
    pub typecode: u8,
    /// The Item's type, named for readers.
    #[serde(rename = "type")]
    pub kind: String,
    /// The receiver's transmission key, in hex.
    pub pk_d: String,
    /// The secret that opens the output, in hex.
    pub secret: String,
}

/// An Item with a typecode the tools do not know.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints)]
pub struct UnknownItem {
    /// The Item's typecode.
    pub typecode: u64,
    /// The Item's body, in hex.
    pub body: String,
}

/// What a Disclosure string carries.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints)]
pub struct DecodeReport {
    /// The network of the transaction.
    pub network: Network,
    /// The transaction.
    pub txid: String,
    /// The Output Items.
    pub items: Vec<ItemReport>,
    /// The Items a Verifier skips.
    pub unknown_items: Vec<UnknownItem>,
}

/// The result of checking one Item against the transaction.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum VerifyItem {
    /// The Item opened its output.
    Verified {
        /// The position of the output in the Sapling outputs.
        index: u32,
        /// The note value in zatoshis.
        value: u64,
        /// The receiver's address.
        address: String,
        /// The memo, as [`view::memo_text`] renders it.
        memo: String,
        /// Whether the receiver's incoming viewing key decrypts the output.
        receiver_decryptable: bool,
    },
    /// The Item does not match the transaction.
    Failed {
        /// The position the Item names.
        index: u32,
        /// Why the Item failed.
        error: String,
    },
}

/// The result of verifying a Disclosure, one entry per Item.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints)]
pub struct VerifyReport {
    /// The transaction.
    pub txid: String,
    /// The Items with their results.
    pub items: Vec<VerifyItem>,
}

/// A Sapling output the key can open.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints)]
pub struct OutputInfo {
    /// The position of the output in the Sapling outputs.
    pub index: u32,
    /// The side the key is on.
    pub role: Role,
    /// The key scope that opened the output.
    pub scope: Scope,
    /// The note value in zatoshis.
    pub value: u64,
    /// Another output to the same address carries the same secret.
    pub secret_reused: bool,
}

/// The outputs a key opens in a transaction, and the Disclosure of the chosen ones.
#[derive(Serialize, Tsify)]
#[tsify(large_number_types_as_bigints, missing_as_null)]
pub struct CreateReport {
    /// Every output the key opens, in bundle order.
    pub outputs: Vec<OutputInfo>,
    /// The secret-reuse warnings of the chosen outputs.
    pub warnings: Vec<String>,
    /// The Disclosure string, or null when no output was chosen.
    pub disclosure: Option<String>,
}

/// Decodes a Disclosure string into the report of what it carries.
pub fn decode_report(disclosure: &str) -> Result<DecodeReport, Error> {
    let disclosure = input::decode(disclosure)?;
    Ok(DecodeReport {
        network: disclosure.network().into(),
        txid: disclosure.txid().to_string(),
        items: disclosure
            .items()
            .iter()
            .map(|item| ItemReport {
                index: item.index,
                typecode: item.kind.typecode(),
                kind: view::kind_name(item.kind),
                pk_d: hex::encode(item.pk_d),
                secret: hex::encode(item.secret),
            })
            .collect(),
        unknown_items: disclosure
            .unknown_items()
            .iter()
            .map(|u| UnknownItem {
                typecode: u.typecode,
                body: hex::encode(&u.body),
            })
            .collect(),
    })
}

/// Verifies a Disclosure against a transaction mined at `height`, on `network` (`main`,
/// `test` or `regtest`). The transaction bytes may be hex or raw. The report has one entry
/// per Item.
pub fn verify_report(
    tx_bytes: &[u8],
    height: u32,
    network_name: &str,
    disclosure: &str,
) -> Result<VerifyReport, Error> {
    let params = params_for(network_name)?;
    let height = BlockHeight::from_u32(height);
    let bytes = tx::from_hex_or_bytes(tx_bytes.to_vec())?;
    let parsed = tx::parse(&params, &bytes, height)?;
    let disclosure = input::decode(disclosure)?;
    let report = check::check(&params, height, &parsed, &disclosure)?;
    Ok(VerifyReport {
        txid: disclosure.txid().to_string(),
        items: report
            .items
            .iter()
            .map(|(item, result)| match result {
                Err(e) => VerifyItem::Failed {
                    index: item.index,
                    error: e.to_string(),
                },
                Ok(note) => VerifyItem::Verified {
                    index: item.index,
                    value: note.value,
                    address: view::receiver_address(&params, note),
                    memo: view::memo_text(&note.memo),
                    receiver_decryptable: note.receivable(),
                },
            })
            .collect(),
    })
}

/// Lists the Sapling outputs a UFVK can open in a transaction, and builds the Disclosure for
/// the requested ones. `outputs` holds indexes into the Sapling outputs and is used only
/// when `all` is false. Internal-scope outputs (change) are refused unless
/// `allow_internal` is given. The report carries the output list, the secret-reuse
/// warnings of the chosen outputs, and the Disclosure string when an output was chosen.
pub fn create_report(
    ufvk: &str,
    tx_bytes: &[u8],
    height: u32,
    network_name: &str,
    all: bool,
    allow_internal: bool,
    outputs: Vec<u32>,
) -> Result<CreateReport, Error> {
    let params = params_for(network_name)?;
    let height = BlockHeight::from_u32(height);
    let bytes = tx::from_hex_or_bytes(tx_bytes.to_vec())?;
    let parsed = tx::parse(&params, &bytes, height)?;
    let key = produce::decode_ufvk(&params, ufvk)?;
    let found = produce::discover(&params, height, &parsed, &key)?;

    let chosen = if all || !outputs.is_empty() {
        produce::select(&found, all, allow_internal, &outputs)?
    } else {
        vec![]
    };
    let warnings = chosen
        .iter()
        .filter(|d| d.secret_reused)
        .map(|d| {
            format!(
                "{} shares its secret with another output to the same address",
                zdisclosure_core::output_name(&d.item)
            )
        })
        .collect();

    let disclosure = if chosen.is_empty() {
        None
    } else {
        Some(
            produce::build(
                &params,
                &parsed,
                chosen.iter().map(|d| d.item.clone()).collect(),
            )?
            .encode(),
        )
    };
    Ok(CreateReport {
        outputs: found
            .iter()
            .map(|d| OutputInfo {
                index: d.item.index,
                role: d.role.into(),
                scope: d.scope.into(),
                value: d.value,
                secret_reused: d.secret_reused,
            })
            .collect(),
        warnings,
        disclosure,
    })
}

/// The browser binding for [`decode_report`]. A failure is thrown as an `Error`.
#[wasm_bindgen]
pub fn decode(disclosure: &str) -> Result<Ts<DecodeReport>, JsError> {
    Ok(decode_report(disclosure)?.into_ts()?)
}

/// The browser binding for [`verify_report`]. `tx` is a `Uint8Array` of hex or raw bytes.
#[wasm_bindgen]
pub fn verify(
    tx: &[u8],
    height: u32,
    network: &str,
    disclosure: &str,
) -> Result<Ts<VerifyReport>, JsError> {
    Ok(verify_report(tx, height, network, disclosure)?.into_ts()?)
}

/// The browser binding for [`create_report`]. `tx` is a `Uint8Array` of hex or raw bytes,
/// `outputs` a `Uint32Array` of indexes into the Sapling outputs.
#[wasm_bindgen]
pub fn create(
    ufvk: &str,
    tx: &[u8],
    height: u32,
    network: &str,
    all: bool,
    allow_internal: bool,
    outputs: Vec<u32>,
) -> Result<Ts<CreateReport>, JsError> {
    Ok(create_report(ufvk, tx, height, network, all, allow_internal, outputs)?.into_ts()?)
}

#[cfg(test)]
mod tests {
    use super::{Network, decode_report};
    use zcash_disclosure::{Disclosure, Item, Kind};
    use zcash_protocol::{TxId, consensus::NetworkType};

    #[test]
    fn a_disclosure_decodes_to_a_report() {
        let d = Disclosure::new(
            NetworkType::Main,
            TxId::from_bytes([7; 32]),
            vec![Item {
                kind: Kind::Sapling,
                index: 3,
                pk_d: [1; 32],
                secret: [2; 32],
            }],
        )
        .unwrap();
        let report = decode_report(&d.encode()).unwrap();
        assert_eq!(report.network, Network::Main);
        assert_eq!(report.items[0].index, 3);
        assert_eq!(report.items[0].typecode, 2);
    }

    #[test]
    fn broken_strings_are_errors() {
        assert!(decode_report("zdu1qqqq").is_err());
    }
}
