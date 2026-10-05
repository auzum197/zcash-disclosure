//! `zdisclosure-create`: produces a Disclosure for Sapling outputs of a transaction.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use zcash_disclosure::{Disclosable, Disclosure, set};
use zcash_protocol::TxId;
use zdisclosure_cli::network::NetworkArgs;
use zdisclosure_cli::source::{Source, TxArgs};
use zdisclosure_cli::{Error, Exit, Io, input, write_err};
use zdisclosure_core::input::covered;
use zdisclosure_core::output_name;
use zdisclosure_core::produce;
use zdisclosure_core::view::{role_name, scope_name};

const PROGRAM: &str = "zdisclosure-create";

/// Produces a Disclosure for chosen Sapling outputs of a transaction, from a UFVK.
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

    /// Print the Sapling outputs the key can disclose, one per line, and exit. The columns
    /// are index, role, scope, value in zatoshis, and flags.
    #[arg(long, conflicts_with_all = ["output", "all", "allow_internal", "out", "qr"])]
    pub list: bool,

    /// An output to disclose, by its index in the Sapling outputs. Can repeat.
    #[arg(long, value_name = "INDEX", conflicts_with = "all")]
    pub output: Vec<u32>,

    /// Disclose every external-scope output the key can open.
    #[arg(long)]
    pub all: bool,

    /// Allow internal-scope (change) outputs.
    #[arg(long)]
    pub allow_internal: bool,

    /// Merge the Disclosure into this .zdisc set instead of printing it.
    #[arg(long, value_name = "FILE", conflicts_with = "qr")]
    pub out: Option<PathBuf>,

    /// Print the Disclosure as a QR code.
    #[arg(long)]
    pub qr: bool,
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
    let ufvk = produce::decode_ufvk(&params, &ufvk_text).map_err(Error::usage)?;

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

    let found =
        produce::discover(&params, fetched.height, &fetched.tx, &ufvk).map_err(Error::usage)?;

    if cli.list {
        for d in &found {
            list_line(io.stdout, d).map_err(write_err)?;
        }
        return Ok(Exit::Ok);
    }

    let chosen = select(cli, &found)?;
    for d in &chosen {
        if d.secret_reused {
            io.warn(
                PROGRAM,
                format!(
                    "{} shares its secret with another output to the same address",
                    output_name(&d.item)
                ),
            );
        }
    }

    let disclosure = produce::build(
        &params,
        &fetched.tx,
        chosen.into_iter().map(|d| d.item.clone()).collect(),
    )
    .map_err(Error::failed)?;

    if let Some(path) = &cli.out {
        if path.as_path() == Path::new("-") {
            return Err(Error::usage("--out does not take -"));
        }
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
        "{}\t{}\t{}\t{}\t{}",
        d.item.index,
        role_name(d.role),
        scope_name(d.scope),
        d.value,
        if d.secret_reused {
            "secret-reused"
        } else {
            "-"
        },
    )
}

fn select<'a>(cli: &Cli, found: &'a [Disclosable]) -> Result<Vec<&'a Disclosable>, Error> {
    produce::select(found, cli.all, cli.allow_internal, &cli.output).map_err(Error::failed)
}

/// Merges `disclosure` into the set at `path`, creating the file when it is missing.
fn merge_into(path: &Path, disclosure: Disclosure) -> Result<(), Error> {
    let shown = path.display();
    let mut lines = match fs::read_to_string(path) {
        Ok(text) if text.is_empty() => vec![],
        Ok(text) => {
            let parsed = set::parse(&text).map_err(|e| Error::usage(format!("{shown}: {e}")))?;
            parsed
                .iter()
                .try_for_each(|d| covered(d).map_err(Error::usage))?;
            parsed
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(Error::usage(format!("cannot read {shown}: {e}"))),
    };

    match lines.iter_mut().find(|d| d.txid() == disclosure.txid()) {
        None => lines.push(disclosure),
        Some(existing) => *existing = merged(existing, &disclosure, &shown.to_string())?,
    }
    let text = set::format(&lines).map_err(|e| Error::usage(format!("{shown}: {e}")))?;

    // The temp file is created in place, so a name planted beforehand fails the run instead
    // of being written through. It holds payment secrets and starts owner-only.
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map_or("zdisc".into(), |n| n.to_string_lossy().into_owned());
    let mut placed = None;
    for n in 0..64u32 {
        let candidate = dir.join(format!(".{name}.{}.{}.tmp", std::process::id(), n));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => {
                placed = Some((candidate, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::usage(format!("cannot write {shown}: {e}"))),
        }
    }
    let Some((tmp, mut file)) = placed else {
        return Err(Error::usage(format!(
            "cannot create a temporary file next to {shown}"
        )));
    };
    let written = file.write_all(text.as_bytes()).and_then(|()| file.flush());
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(Error::usage(format!("cannot write {shown}: {e}")));
    }
    drop(file);

    // A set we merge into keeps the mode the user gave it.
    let keep = match fs::symlink_metadata(path) {
        Ok(meta) => fs::set_permissions(&tmp, meta.permissions()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    };
    if let Err(e) = keep.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(Error::usage(format!("cannot write {shown}: {e}")));
    }
    Ok(())
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
        match items.iter().find(|i| i.index == item.index) {
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
    fn merging_adds_items_and_lines() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("zdisc-merge-{}.zdisc", std::process::id()));
        let _ = fs::remove_file(&path);
        let mode = || fs::metadata(&path).unwrap().permissions().mode() & 0o777;

        merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 5)])).unwrap();
        assert_eq!(mode(), 0o600);
        let mut loose = fs::metadata(&path).unwrap().permissions();
        loose.set_mode(0o640);
        fs::set_permissions(&path, loose).unwrap();

        merge_into(
            &path,
            disclosure(1, vec![item(Kind::SaplingLegacySender, 2, 6)]),
        )
        .unwrap();
        merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 5)])).unwrap();
        merge_into(&path, disclosure(2, vec![item(Kind::Sapling, 0, 7)])).unwrap();
        assert_eq!(mode(), 0o640);

        let set = set::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            set,
            vec![
                disclosure(
                    1,
                    vec![
                        item(Kind::Sapling, 0, 5),
                        item(Kind::SaplingLegacySender, 2, 6)
                    ]
                ),
                disclosure(2, vec![item(Kind::Sapling, 0, 7)]),
            ]
        );

        let conflict = merge_into(&path, disclosure(1, vec![item(Kind::Sapling, 0, 9)]));
        assert_eq!(conflict.unwrap_err().exit, Exit::Failed);
        let conflict = merge_into(
            &path,
            disclosure(1, vec![item(Kind::SaplingLegacyReceiver, 2, 6)]),
        );
        assert_eq!(conflict.unwrap_err().exit, Exit::Failed);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_set_with_another_pool_is_not_merged_into() {
        let path = std::env::temp_dir().join(format!("zdisc-pool-{}.zdisc", std::process::id()));
        let orchard = disclosure(1, vec![item(Kind::Orchard, 0, 5)]);
        fs::write(&path, set::format(&[orchard]).unwrap()).unwrap();

        let refused = merge_into(&path, disclosure(2, vec![item(Kind::Sapling, 0, 7)]));
        assert_eq!(refused.unwrap_err().exit, Exit::Usage);
        fs::remove_file(path).unwrap();
    }
}
