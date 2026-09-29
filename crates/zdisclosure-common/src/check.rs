//! Checking Disclosures against their transactions, for `zdisclosure-verify` and
//! `zdisclosure-inspect`.

use std::io::Read;

use zcash_disclosure::{Disclosure, Report, verify};

use crate::network::Params;
use crate::source::{Fetched, Source, TxArgs};
use crate::{Error, Exit};

/// A Disclosure checked against its transaction.
#[derive(Debug)]
pub struct Checked {
    /// The transaction and its place in the chain.
    pub fetched: Fetched,
    /// The result per Item.
    pub report: Report,
}

/// Rejects `-` for both the Disclosure input and `--tx`, before either is read.
pub fn one_stdin(input: &str, tx: &TxArgs) -> Result<(), Error> {
    if input == "-" && tx.reads_stdin() {
        return Err(Error::usage("the input and --tx - cannot both read stdin"));
    }
    Ok(())
}

/// Opens the transaction source for `count` Disclosures. A raw transaction covers one.
pub fn open(tx: &TxArgs, count: usize, stdin: &mut dyn Read) -> Result<Option<Source>, Error> {
    let source = Source::open(tx, stdin)?;
    if count > 1 && matches!(source, Some(Source::Raw { .. })) {
        return Err(Error::usage(format!(
            "--tx gives one transaction, and the input holds {count} disclosures"
        )));
    }
    Ok(source)
}

/// Obtains the transaction `disclosure` covers and verifies the Disclosure against it.
pub fn check(source: &Source, params: &Params, disclosure: &Disclosure) -> Result<Checked, Error> {
    let fetched = source.fetch(params, Some(disclosure.txid()))?;
    let report = verify(params, fetched.height, &fetched.tx, disclosure)
        .map_err(|e| Error::failed(format!("{}: {e}", disclosure.txid())))?;
    Ok(Checked { fetched, report })
}

/// Combines the statuses of several checks. The higher code wins, so an unobtainable
/// transaction outranks a failed Item.
pub fn worst(a: Exit, b: Exit) -> Exit {
    if (b as u8) > (a as u8) { b } else { a }
}
