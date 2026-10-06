//! Producing a Disclosure from a viewing key and the transaction it opens.

use zcash_disclosure::{Disclosable, Disclosure, Item, ViewingKeys};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, Parameters};
use zip32::Scope;

use crate::Error;
use crate::network::Params;

/// Decodes a UFVK text. The decoder's message is left out, so that no part of the key
/// reaches an error.
pub fn decode_ufvk(params: &Params, text: &str) -> Result<UnifiedFullViewingKey, Error> {
    UnifiedFullViewingKey::decode(params, text.trim())
        .map_err(|_| Error::InvalidUfvk(params.network_type()))
}

/// The Sapling outputs of a transaction that `key` can open, in bundle order.
pub fn discover(
    params: &Params,
    height: BlockHeight,
    tx: &Transaction,
    key: &UnifiedFullViewingKey,
) -> Result<Vec<Disclosable>, Error> {
    let sapling = key.sapling().ok_or(Error::NoSaplingKey)?;
    let keys = ViewingKeys {
        sapling: Some(sapling),
        orchard: None,
    };
    Ok(zcash_disclosure::discover(params, height, tx, keys))
}

/// Selects the outputs to disclose, named by their index in the Sapling outputs.
/// Internal-scope outputs (change) are refused unless `allow_internal` is given, and `--all` takes every external output.
pub fn select<'a>(
    found: &'a [Disclosable],
    all: bool,
    allow_internal: bool,
    outputs: &[u32],
) -> Result<Vec<&'a Disclosable>, Error> {
    if all {
        let chosen: Vec<_> = found
            .iter()
            .filter(|d| allow_internal || d.scope == Scope::External)
            .collect();
        if chosen.is_empty() {
            return Err(Error::NothingDisclosable);
        }
        return Ok(chosen);
    }

    let mut chosen: Vec<&Disclosable> = vec![];
    for index in outputs {
        let d = found
            .iter()
            .find(|d| d.item.index == *index)
            .ok_or(Error::OutputNotOpened(*index))?;
        if d.scope == Scope::Internal && !allow_internal {
            return Err(Error::InternalOutput(d.item.index));
        }
        if !chosen.iter().any(|c| c.item == d.item) {
            chosen.push(d);
        }
    }
    Ok(chosen)
}

/// Builds the Disclosure for `items`.
pub fn build(params: &Params, tx: &Transaction, items: Vec<Item>) -> Result<Disclosure, Error> {
    Disclosure::new(params.network_type(), tx.txid(), items)
        .map_err(|e| Error::UnbuildableDisclosure(e.to_string()))
}
