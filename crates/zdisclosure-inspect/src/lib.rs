//! `zdisclosure-inspect`: prints a Disclosure and, given its transaction, what it opens.

use std::io::{self, Write};

use clap::Parser;
use zcash_disclosure::{DisclosedNote, Disclosure, Item, ItemError, Kind, Pool};
use zcash_keys::address::{Address, UnifiedAddress};
use zcash_protocol::consensus::NetworkType;
use zcash_protocol::memo::{Memo, MemoBytes};
use zdisclosure_common::check::{self, Checked, worst};
use zdisclosure_common::network::{NetworkArgs, Params};
use zdisclosure_common::source::TxArgs;
use zdisclosure_common::{Error, Exit, Io, input, output_name, write_err};

const PROGRAM: &str = "zdisclosure-inspect";

const CAVEAT: &str = "A disclosure does not show block inclusion, transaction validity, spend \
                      status or authorship.";

/// Prints the contents of shielded note Disclosures.
///
/// Without a transaction, the Disclosures are decoded offline. With --tx or --lightwalletd,
/// each one is also verified, and the value, receiver, memo and result of each Item are
/// printed. The exit status follows zdisclosure-verify.
#[derive(Debug, Parser)]
#[command(name = PROGRAM, version)]
pub struct Cli {
    /// A Disclosure string, `-` for stdin, or the path of a .zdisc file.
    pub input: String,

    #[command(flatten)]
    #[allow(missing_docs)]
    pub network: NetworkArgs,

    #[command(flatten)]
    #[allow(missing_docs)]
    pub tx: TxArgs,
}

/// Runs the tool and returns its exit status.
pub fn run(cli: Cli, io: &mut Io<'_>) -> Exit {
    let result = inspect(&cli, io);
    io.finish(PROGRAM, result)
}

fn inspect(cli: &Cli, io: &mut Io<'_>) -> Result<Exit, Error> {
    let params = cli.network.params()?;
    check::one_stdin(&cli.input, &cli.tx)?;
    let disclosures = input::read_disclosures(&cli.input, io.stdin)?;
    let source = check::open(&cli.tx, disclosures.len(), io.stdin)?;

    let mut exit = Exit::Ok;
    for (i, disclosure) in disclosures.iter().enumerate() {
        if i > 0 {
            writeln!(io.stdout).map_err(write_err)?;
        }
        let checked = match &source {
            None => None,
            Some(source) => match check::check(source, &params, disclosure) {
                Ok(checked) => Some(checked),
                Err(e) if e.exit == Exit::Usage => return Err(e),
                Err(e) => {
                    io.warn(PROGRAM, &e);
                    exit = worst(exit, e.exit);
                    None
                }
            },
        };
        if checked
            .as_ref()
            .is_some_and(|c| c.report.items.iter().any(|(_, r)| r.is_err()))
        {
            exit = worst(exit, Exit::Failed);
        }
        print(io.stdout, &params, disclosure, checked.as_ref()).map_err(write_err)?;
    }
    if source.is_some() {
        writeln!(io.stdout, "\n{CAVEAT}").map_err(write_err)?;
    }
    Ok(exit)
}

fn print(
    out: &mut dyn Write,
    params: &Params,
    disclosure: &Disclosure,
    checked: Option<&Checked>,
) -> io::Result<()> {
    writeln!(out, "network   {}", network_name(disclosure.network()))?;
    writeln!(out, "txid      {}", disclosure.txid())?;
    if let Some(c) = checked {
        match c.fetched.depth {
            Some(depth) => writeln!(out, "height    {} ({depth} deep)", c.fetched.height)?,
            None => writeln!(out, "height    {}", c.fetched.height)?,
        }
    }

    for (n, item) in disclosure.items().iter().enumerate() {
        writeln!(
            out,
            "item {}    {}  {}",
            n + 1,
            output_name(item),
            kind_name(item.kind)
        )?;
        writeln!(out, "  pk_d    {}", hex::encode(item.pk_d))?;
        writeln!(out, "  secret  {}", hex::encode(item.secret))?;
        let result = checked.and_then(|c| c.report.items.iter().find(|(i, _)| i == item));
        if let Some((_, result)) = result {
            print_result(out, params, item, result)?;
        }
    }
    for unknown in disclosure.unknown_items() {
        writeln!(
            out,
            "skipped   typecode {:#04x}, body {}",
            unknown.typecode,
            hex::encode(&unknown.body)
        )?;
    }
    Ok(())
}

fn print_result(
    out: &mut dyn Write,
    params: &Params,
    item: &Item,
    result: &Result<DisclosedNote, ItemError>,
) -> io::Result<()> {
    let note = match result {
        Ok(note) => note,
        Err(e) => return writeln!(out, "  result  FAILED: {e}"),
    };
    writeln!(out, "  result  verified")?;
    writeln!(
        out,
        "  value   {}.{:08} ZEC ({} zatoshis)",
        note.value / 100_000_000,
        note.value % 100_000_000,
        note.value
    )?;
    writeln!(
        out,
        "  address {}",
        receiver(params, item.kind.pool(), note)
    )?;
    writeln!(out, "  memo    {}", memo(&note.memo))?;
    if note.receivable() {
        writeln!(out, "  receipt the receiver's wallet decrypts this output")
    } else {
        writeln!(out, "  receipt note contents verified, receipt unverified")
    }
}

fn network_name(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Main => "main",
        NetworkType::Test => "test",
        NetworkType::Regtest => "regtest",
    }
}

fn kind_name(kind: Kind) -> String {
    let name = match kind {
        Kind::Sapling => "Sapling",
        Kind::Orchard => "Orchard",
        Kind::SaplingLegacySender => "Sapling before ZIP 212, sender",
        Kind::SaplingLegacyReceiver => "Sapling before ZIP 212, receiver",
        Kind::Ironwood => "Ironwood",
    };
    format!("type {:#04x} ({name})", kind.typecode())
}

/// The receiver as an address. Ironwood uses Orchard addresses, so both are shown as a
/// unified address with one Orchard receiver.
fn receiver(params: &Params, pool: Pool, note: &DisclosedNote) -> String {
    let mut raw = [0; 43];
    raw[..11].copy_from_slice(&note.diversifier);
    raw[11..].copy_from_slice(&note.pk_d);
    let encoded = match pool {
        Pool::Sapling => sapling::PaymentAddress::from_bytes(&raw)
            .map(|a| Address::Sapling(Box::new(a)).encode(params)),
        Pool::Orchard | Pool::Ironwood => orchard::Address::from_raw_address_bytes(&raw)
            .into_option()
            .and_then(|a| UnifiedAddress::from_receivers(Some(a), None, None, None, None))
            .map(|ua| ua.encode(params)),
    };
    encoded.unwrap_or_else(|| format!("raw {}", hex::encode(raw)))
}

fn memo(bytes: &[u8; 512]) -> String {
    let parsed = MemoBytes::from_bytes(bytes)
        .ok()
        .and_then(|m| Memo::try_from(m).ok());
    match parsed {
        Some(Memo::Empty) => "empty".to_owned(),
        Some(Memo::Text(text)) => format!("{:?}", &*text),
        _ => {
            let end = bytes.iter().rposition(|&b| b != 0).map_or(1, |i| i + 1);
            format!("hex {}", hex::encode(&bytes[..end]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memo;

    #[test]
    fn memos_render_by_kind() {
        let mut empty = [0; 512];
        empty[0] = 0xf6;
        assert_eq!(memo(&empty), "empty");

        let mut text = [0; 512];
        text[..5].copy_from_slice(b"Rent\n");
        assert_eq!(memo(&text), "\"Rent\\n\"");

        let mut arbitrary = [0; 512];
        arbitrary[..3].copy_from_slice(&[0xff, 0x01, 0x02]);
        assert_eq!(memo(&arbitrary), "hex ff0102");
    }
}
