//! Verifying a Disclosure against the transaction it covers.

use zcash_disclosure::{Disclosure, Report, verify};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BlockHeight;

use crate::Error;
use crate::network::Params;

/// Verifies `disclosure` against `tx`, the transaction mined at `height`. The Sapling spend
/// verifying key is left out, since only a signed Disclosure needs it.
pub fn check(
    params: &Params,
    height: BlockHeight,
    tx: &Transaction,
    disclosure: &Disclosure,
) -> Result<Report, Error> {
    verify(params, height, tx, disclosure, None)
        .map_err(|e| Error::new(format!("{}: {e}", disclosure.txid())))
}
