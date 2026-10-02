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

pub mod check;
pub mod input;
pub mod network;
pub mod produce;
pub mod tx;
pub mod view;

/// An error message from the core. It carries no process meaning; the tools map it onto an
/// exit status where it is raised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    /// Wraps `message` as the error of a failed operation.
    pub fn new(message: impl fmt::Display) -> Self {
        Self(message.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// The output an Item points at, as `output <index>`. The index counts the Sapling outputs
/// of the transaction.
pub fn output_name(item: &Item) -> String {
    format!("output {}", item.index)
}
