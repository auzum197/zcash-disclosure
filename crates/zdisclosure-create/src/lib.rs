//! `zdisclosure-create`: produces a shielded note Disclosure for outputs of a transaction.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Parser;
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use zcash_disclosure::{Disclosable, Disclosure, Pool, Role, ViewingKeys, discover, set};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_protocol::TxId;
use zcash_protocol::consensus::Parameters;
use zdisclosure_common::network::NetworkArgs;
use zdisclosure_common::source::{Source, TxArgs};
use zdisclosure_common::{Error, Exit, Io, input, output_name, pool_name, write_err};
use zip32::Scope;

const PROGRAM: &str = "zdisclosure-create";

/// Produces a Disclosure for chosen shielded outputs of a transaction, from a UFVK.
///
/// A Disclosure lets any holder decrypt and check those outputs. It is unsigned and cannot
/// be revoked once shared. It says nothing about block inclusion, transaction validity,
/// spend status or authorship.
#[derive(Debug, Parser)]
#[command(name = PROGRAM, version)]
pub struct Cli {
    #[command(flatten)]
    #[allow(missing_docs)]
    pub network: NetworkArgs,

    #[command(flatten)]
    #[allow(missing_docs)]
    pub tx: TxArgs,

    /// The transaction to fetch from --lightwalletd, in display byte order.
    #[arg(long, requires = "lightwalletd")]
    pub txid: Option<String>,

    /// A file that holds the unified full viewing key. `-` reads stdin.
    #[arg(long, value_name = "PATH")]
    pub ufvk_file: String,

    /// Print the outputs the key can disclose, one per line, and exit. The columns are pool,
    /// index, role, scope, value in zatoshis, and flags.
    #[arg(long, conflicts_with_all = ["output", "all", "allow_internal", "out", "qr"])]
    pub list: bool,

    /// An output to disclose, as <pool>:<index> with pool sapling, orchard or ironwood. Can
    /// repeat.
    #[arg(long, value_name = "POOL:INDEX", conflicts_with = "all")]
    pub output: Vec<OutputRef>,

    /// Disclose every external-scope output the key can open.
    #[arg(long)]
    pub all: bool,

    /// Allow change and migration outputs. Their Items link to every other change Item of
    /// the account.
    #[arg(long)]
    pub allow_internal: bool,

    /// Merge the Disclosure into this .zdisc set instead of printing it.
    #[arg(long, value_name = "FILE", conflicts_with = "qr")]
    pub out: Option<PathBuf>,

    /// Print the Disclosure as a QR code.
    #[arg(long)]
    pub qr: bool,
}

/// An output named on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputRef {
    /// The bundle.
    pub pool: Pool,
    /// The position in the bundle.
    pub index: u32,
}

impl FromStr for OutputRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let (pool, index) = s.split_once(':').ok_or("expected <pool>:<index>")?;
        let pool = match pool {
            "sapling" => Pool::Sapling,
            "orchard" => Pool::Orchard,
            "ironwood" => Pool::Ironwood,
            other => return Err(format!("unknown pool {other:?}")),
        };
        let index = index
            .parse()
            .map_err(|_| format!("invalid index {index:?}"))?;
        Ok(OutputRef { pool, index })
    }
}

/// Runs the tool and returns its exit status.
pub fn run(cli: Cli, io: &mut Io<'_>) -> Exit {
    let result = create(&cli, io);
    io.finish(PROGRAM, result)
}

fn create(cli: &Cli, io: &mut Io<'_>) -> Result<Exit, Error> {
    let params = cli.network.params()?;
    if cli.ufvk_file == "-" && cli.tx.reads_stdin() {
        return Err(Error::usage(
            "--ufvk-file - and --tx - cannot both read stdin",
        ));
    }
    if !cli.list && cli.output.is_empty() && !cli.all {
        return Err(Error::usage(
            "choose outputs with --output or --all, or pass --list",
        ));
    }

    let ufvk_text = String::from_utf8(input::read_bytes(&cli.ufvk_file, io.stdin)?)
        .map_err(|_| Error::usage("the UFVK file is not UTF-8 text"))?;
    // The decoder's message is left out, so that no part of the key reaches stderr.
    let ufvk = UnifiedFullViewingKey::decode(&params, ufvk_text.trim()).map_err(|_| {
        Error::usage(format!(
            "the UFVK file does not hold a unified full viewing key for {:?}",
            params.network_type()
        ))
    })?;

    let source = Source::open(&cli.tx, io.stdin)?
        .ok_or_else(|| Error::usage("give --tx and --height, or --lightwalletd and --txid"))?;
    let txid = match (&source, &cli.txid) {
        (Source::Lightwalletd(_), None) => return Err(Error::usage("--lightwalletd needs --txid")),
        (_, txid) => txid
            .as_deref()
            .map(|s| TxId::from_hex(s).ok_or_else(|| Error::usage(format!("invalid txid {s}"))))
            .transpose()?,
    };
    let fetched = source.fetch(&params, txid)?;

    let keys = ViewingKeys {
        sapling: ufvk.sapling(),
        orchard: ufvk.orchard(),
    };
    let found = discover(&params, fetched.height, &fetched.tx, keys);

    if cli.list {
        for d in &found {
            list_line(io.stdout, d).map_err(write_err)?;
        }
        return Ok(Exit::Ok);
    }

    let chosen = select(cli, &found)?;
    for d in &chosen {
        if d.scope == Scope::Internal {
            io.warn(
                PROGRAM,
                format!(
                    "{} is a change or migration output. Its Item links to every other change \
                     Item of this account and exposes the internal scope to an adversary that \
                     computes discrete logarithms.",
                    output_name(&d.item)
                ),
            );
        }
        if d.secret_reused {
            io.warn(
                PROGRAM,
                format!(
                    "{} shares its secret with another output of this transaction to the same \
                     address. A holder of this Disclosure can open that output too.",
                    output_name(&d.item)
                ),
            );
        }
    }

    let disclosure = Disclosure::new(
        params.network_type(),
        fetched.tx.txid(),
        chosen.into_iter().map(|d| d.item.clone()).collect(),
    )
    .map_err(|e| Error::failed(format!("cannot build the disclosure: {e}")))?;

    if let Some(path) = &cli.out {
        merge_into(path, disclosure)?;
    } else if cli.qr {
        let code = QrCode::new(disclosure.encode().as_bytes())
            .map_err(|e| Error::failed(format!("the disclosure does not fit a QR code: {e}")))?;
        let art = code
            .render::<Dense1x2>()
            .dark_color(Dense1x2::Light)
            .light_color(Dense1x2::Dark)
            .build();
        writeln!(io.stdout, "{art}").map_err(write_err)?;
    } else {
        writeln!(io.stdout, "{}", disclosure.encode()).map_err(write_err)?;
    }
    Ok(Exit::Ok)
}

fn list_line(out: &mut dyn Write, d: &Disclosable) -> std::io::Result<()> {
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}",
        pool_name(d.item.kind.pool()),
        d.item.index,
        match d.role {
            Role::Receiver => "receiver",
            Role::Sender => "sender",
        },
        match d.scope {
            Scope::External => "external",
            Scope::Internal => "internal",
        },
        d.value,
        if d.secret_reused {
            "secret-reused"
        } else {
            "-"
        },
    )
}

fn select<'a>(cli: &Cli, found: &'a [Disclosable]) -> Result<Vec<&'a Disclosable>, Error> {
    if cli.all {
        let chosen: Vec<_> = found
            .iter()
            .filter(|d| cli.allow_internal || d.scope == Scope::External)
            .collect();
        if chosen.is_empty() {
            return Err(Error::failed("the key opens no output to disclose"));
        }
        return Ok(chosen);
    }

    let mut chosen: Vec<&Disclosable> = vec![];
    for r in &cli.output {
        let d = found
            .iter()
            .find(|d| d.item.kind.pool() == r.pool && d.item.index == r.index)
            .ok_or_else(|| {
                Error::failed(format!(
                    "the key does not open {}:{}",
                    pool_name(r.pool),
                    r.index
                ))
            })?;
        if d.scope == Scope::Internal && !cli.allow_internal {
            return Err(Error::failed(format!(
                "{} is a change or migration output. Pass --allow-internal to disclose it.",
                output_name(&d.item)
            )));
        }
        if !chosen.iter().any(|c| c.item == d.item) {
            chosen.push(d);
        }
    }
    Ok(chosen)
}

/// Merges `disclosure` into the set at `path`, creating the file when it is missing.
fn merge_into(path: &Path, disclosure: Disclosure) -> Result<(), Error> {
    let shown = path.display();
    let mut lines = match fs::read_to_string(path) {
        Ok(text) if text.is_empty() => vec![],
        Ok(text) => set::parse(&text).map_err(|e| Error::usage(format!("{shown}: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(Error::usage(format!("cannot read {shown}: {e}"))),
    };

    match lines.iter_mut().find(|d| d.txid() == disclosure.txid()) {
        None => lines.push(disclosure),
        Some(existing) => *existing = merged(existing, &disclosure, &shown.to_string())?,
    }
    let text = set::format(&lines).map_err(|e| Error::usage(format!("{shown}: {e}")))?;

    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .map_or("zdisc".into(), |n| n.to_string_lossy()),
        std::process::id()
    ));
    fs::write(&tmp, text)
        .and_then(|()| fs::rename(&tmp, path))
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::usage(format!("cannot write {shown}: {e}"))
        })
}

fn merged(existing: &Disclosure, new: &Disclosure, file: &str) -> Result<Disclosure, Error> {
    if !existing.unknown_items().is_empty() {
        return Err(Error::usage(format!(
            "{file}: the line for {} has Items of unknown types and cannot be merged",
            existing.txid()
        )));
    }
    let mut items = existing.items().to_vec();
    for item in new.items() {
        match items
            .iter()
            .find(|i| i.kind.pool() == item.kind.pool() && i.index == item.index)
        {
            None => items.push(item.clone()),
            Some(i) if i == item => {}
            Some(_) => {
                return Err(Error::failed(format!(
                    "{file}: {} already has a different Item for {}",
                    existing.txid(),
                    output_name(item)
                )));
            }
        }
    }
    Disclosure::new(existing.network(), existing.txid(), items)
        .map_err(|e| Error::failed(format!("{file}: cannot merge: {e}")))
}

#[cfg(test)]
mod tests {
    use zcash_disclosure::{Item, Kind};
    use zcash_protocol::consensus::NetworkType;

    use super::*;

    fn item(kind: Kind, index: u32, secret: u8) -> Item {
        Item {
            kind,
            index,
            pk_d: [1; 32],
            secret: [secret; 32],
        }
    }

    fn disclosure(txid: u8, items: Vec<Item>) -> Disclosure {
        Disclosure::new(NetworkType::Main, TxId::from_bytes([txid; 32]), items).unwrap()
    }

    #[test]
    fn output_refs_parse() {
        assert_eq!(
            "orchard:7".parse(),
            Ok(OutputRef {
                pool: Pool::Orchard,
                index: 7
            })
        );
        assert!("sprout:1".parse::<OutputRef>().is_err());
        assert!("sapling".parse::<OutputRef>().is_err());
        assert!("sapling:-1".parse::<OutputRef>().is_err());
    }

    #[test]
    fn merging_adds_items_and_lines() {
        let path = std::env::temp_dir().join(format!("zdisc-merge-{}.zdisc", std::process::id()));
        let _ = fs::remove_file(&path);

        merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 5)])).unwrap();
        merge_into(&path, disclosure(1, vec![item(Kind::Orchard, 2, 6)])).unwrap();
        merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 5)])).unwrap();
        merge_into(&path, disclosure(2, vec![item(Kind::Ironwood, 0, 7)])).unwrap();

        let set = set::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            set,
            vec![
                disclosure(
                    1,
                    vec![item(Kind::Sapling, 0, 5), item(Kind::Orchard, 2, 6)]
                ),
                disclosure(2, vec![item(Kind::Ironwood, 0, 7)]),
            ]
        );

        let conflict = merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 9)]));
        assert_eq!(conflict.unwrap_err().exit, Exit::Failed);
        fs::remove_file(path).unwrap();
    }
}
