//! Decoding, verification and the report fields of the `zdisclosure` tools, free of
//! process I/O, so the browser build shares its logic with the command line.
//!
//! The tools handle unsigned Disclosures of Sapling outputs. [`input`] refuses the signed
//! class and Items of the other pools.
//!
//! - [`input`] turns strings into Disclosures and enforces the whole canonical form.
//! - [`tx`] reads transaction bytes as hex or binary and parses them under a height.
//! - [`check`] verifies a Disclosure against its transaction.
//! - [`produce`] discloses the outputs of a transaction from a viewing key.
//! - [`network`] builds consensus parameters, including regtest schedules.
//! - [`view`] names and formats the fields a report shows.

use std::fmt;

use zcash_disclosure::Item;
use zcash_protocol::TxId;
use zcash_protocol::consensus::NetworkType;

pub mod check;
pub mod input;
pub mod network;
pub mod produce;
pub mod tx;
pub mod view;

/// Why an operation of the core failed. It carries no process meaning; the tools map it
/// onto an exit status where it is raised. Variants that wrap another library's failure
/// hold that failure's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A Disclosure string broke the canonical form.
    InvalidDisclosure(String),
    /// The Disclosure belongs to the signed class.
    SignedClass,
    /// An Item outside the Sapling pool, by its typecode.
    ForeignPool(u8),
    /// Unknown Items repeat a typecode.
    UnsortedItems,
    /// The input held no Disclosure.
    EmptyInput,
    /// A Disclosure set broke the canonical form.
    InvalidSet(String),
    /// Transaction hex did not decode.
    InvalidHex(String),
    /// Transaction bytes did not parse.
    UnparsableTransaction(String),
    /// This many bytes follow the end of the transaction.
    TrailingBytes(usize),
    /// The network name is neither main, test nor regtest.
    UnknownNetwork(String),
    /// The key is not a unified full viewing key of this network.
    InvalidUfvk(NetworkType),
    /// The key has no Sapling component.
    NoSaplingKey,
    /// No output is left to disclose.
    NothingDisclosable,
    /// The key does not open the output at this index.
    OutputNotOpened(u32),
    /// The output at this index is internal-scope (change).
    InternalOutput(u32),
    /// The Disclosure could not be built.
    UnbuildableDisclosure(String),
    /// Verification failed under this txid.
    VerificationFailed {
        /// The transaction the Disclosure covers.
        txid: TxId,
        /// The verifier's message.
        source: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidDisclosure(e) => write!(f, "invalid disclosure: {e}"),
            Error::SignedClass => f.write_str("unsupported disclosure: the signed class"),
            Error::ForeignPool(typecode) => {
                write!(
                    f,
                    "unsupported disclosure: typecode {typecode:#04x} is not a Sapling item"
                )
            }
            Error::UnsortedItems => {
                f.write_str("invalid disclosure: items are not sorted by (typecode, index)")
            }
            Error::EmptyInput => f.write_str("no disclosure in the input"),
            Error::InvalidSet(e) => write!(f, "invalid disclosure set: {e}"),
            Error::InvalidHex(e) => write!(f, "invalid transaction hex: {e}"),
            Error::UnparsableTransaction(e) => write!(f, "cannot parse the transaction: {e}"),
            Error::TrailingBytes(n) => write!(f, "{n} bytes follow the end of the transaction"),
            Error::UnknownNetwork(name) => write!(f, "unknown network {name:?}"),
            Error::InvalidUfvk(network) => {
                write!(
                    f,
                    "the key is not a unified full viewing key for {network:?}"
                )
            }
            Error::NoSaplingKey => f.write_str("the key has no Sapling component"),
            Error::NothingDisclosable => f.write_str("the key opens no output to disclose"),
            Error::OutputNotOpened(index) => write!(f, "the key does not open output {index}"),
            Error::InternalOutput(index) => {
                write!(
                    f,
                    "output {index} is internal-scope and needs --allow-internal"
                )
            }
            Error::UnbuildableDisclosure(e) => write!(f, "cannot build the disclosure: {e}"),
            Error::VerificationFailed { txid, source } => write!(f, "{txid}: {source}"),
        }
    }
}

impl std::error::Error for Error {}

/// The output an Item points at, as `output <index>`. The index counts the Sapling outputs
/// of the transaction.
pub fn output_name(item: &Item) -> String {
    format!("output {}", item.index)
}
