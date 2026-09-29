//! `zdisclosure-verify`: checks a Disclosure and answers with its exit status.

use clap::Parser;
use zdisclosure_common::check::{self, worst};
use zdisclosure_common::network::NetworkArgs;
use zdisclosure_common::source::TxArgs;
use zdisclosure_common::{Error, Exit, Io, input, output_name};

const PROGRAM: &str = "zdisclosure-verify";

/// Checks shielded note Disclosures against the transactions they cover.
///
/// Prints nothing on stdout. The exit status is 0 when every Item verifies, 1 when an Item
/// or a Disclosure fails, 2 on a usage or format error, and 3 when a transaction cannot be
/// obtained. Each failure is described on stderr unless --quiet is given.
///
/// A verified Item binds the note and memo to the output. It says nothing about block
/// inclusion, transaction validity, spend status or authorship.
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

    /// Fail when a transaction is fewer than N blocks deep. A mempool transaction is 0 deep.
    /// Needs --lightwalletd.
    #[arg(long, value_name = "N", requires = "lightwalletd")]
    pub min_confirmations: Option<u64>,

    /// Print nothing on stderr either.
    #[arg(short, long)]
    pub quiet: bool,
}

/// Runs the tool and returns its exit status.
pub fn run(cli: Cli, io: &mut Io<'_>) -> Exit {
    verify_all(&cli, io).unwrap_or_else(|e| {
        report(&cli, io, &e);
        e.exit
    })
}

fn report(cli: &Cli, io: &mut Io<'_>, e: &Error) {
    if !cli.quiet {
        io.warn(PROGRAM, e);
    }
}

fn verify_all(cli: &Cli, io: &mut Io<'_>) -> Result<Exit, Error> {
    let params = cli.network.params()?;
    check::one_stdin(&cli.input, &cli.tx)?;
    let disclosures = input::read_disclosures(&cli.input, io.stdin)?;
    let source = check::open(&cli.tx, disclosures.len(), io.stdin)?
        .ok_or_else(|| Error::usage("give --tx and --height, or --lightwalletd"))?;

    let mut exit = Exit::Ok;
    for disclosure in &disclosures {
        let checked = match check::check(&source, &params, disclosure) {
            Ok(checked) => checked,
            Err(e) if e.exit == Exit::Usage => return Err(e),
            Err(e) => {
                report(cli, io, &e);
                exit = worst(exit, e.exit);
                continue;
            }
        };
        for (item, result) in &checked.report.items {
            if let Err(e) = result {
                report(
                    cli,
                    io,
                    &Error::failed(format!("{} {}: {e}", disclosure.txid(), output_name(item))),
                );
                exit = worst(exit, Exit::Failed);
            }
        }
        if let (Some(min), Some(depth)) = (cli.min_confirmations, checked.fetched.depth)
            && depth < min
        {
            report(
                cli,
                io,
                &Error::failed(format!(
                    "{}: {depth} confirmations, fewer than {min}",
                    disclosure.txid()
                )),
            );
            exit = worst(exit, Exit::Failed);
        }
    }
    Ok(exit)
}
