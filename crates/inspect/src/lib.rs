//! `zdisclosure-inspect`: prints a Disclosure and, given its transaction, what it opens.

use std::fmt;
use std::io::{self, Write};

use clap::Parser;
use zcash_disclosure::{DisclosedNote, Disclosure, ItemError};
use zdisclosure_cli::check::{self, Checked, worst};
use zdisclosure_cli::network::{NetworkArgs, Params};
use zdisclosure_cli::source::TxArgs;
use zdisclosure_cli::{Error, Exit, Io, input, write_err};
use zdisclosure_core::output_name;
use zdisclosure_core::view::{kind_name, memo_text, network_name, receiver_address, zec_amount};

const PROGRAM: &str = "zdisclosure-inspect";

/// Prints the contents of Disclosures of Sapling outputs.
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
    match inspect(&cli, io) {
        Ok(exit) => exit,
        Err(e) => {
            if !cli.quiet && !e.is_quiet() {
                io.warn(PROGRAM, &e);
            }
            e.exit
        }
    }
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
                    if !cli.quiet && !e.is_quiet() {
                        io.warn(PROGRAM, &e);
                    }
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
        if let Some(c) = &checked
            && let (Some(min), Some(depth)) = (cli.min_confirmations, c.fetched.depth)
            && depth < min
        {
            if !cli.quiet {
                io.warn(
                    PROGRAM,
                    format!(
                        "{}: {depth} confirmations, fewer than {min}",
                        disclosure.txid()
                    ),
                );
            }
            exit = worst(exit, Exit::Failed);
        }
        print(io.stdout, &params, disclosure, checked.as_ref()).map_err(write_err)?;
    }
    Ok(exit)
}

/// The column every value starts at. The widest label, `receiver-decryptable`, sets it.
const VALUE_COLUMN: usize = 24;

fn row(out: &mut dyn Write, label: impl fmt::Display, value: impl fmt::Display) -> io::Result<()> {
    writeln!(out, "{:<width$}{}", label, value, width = VALUE_COLUMN - 1)
}

fn sub_row(
    out: &mut dyn Write,
    label: impl fmt::Display,
    value: impl fmt::Display,
) -> io::Result<()> {
    writeln!(
        out,
        "  {:<width$}{}",
        label,
        value,
        width = VALUE_COLUMN - 3
    )
}

fn print(
    out: &mut dyn Write,
    params: &Params,
    disclosure: &Disclosure,
    checked: Option<&Checked>,
) -> io::Result<()> {
    row(out, "network", network_name(disclosure.network()))?;
    row(out, "txid", disclosure.txid())?;
    if let Some(c) = checked {
        match c.fetched.depth {
            Some(depth) => row(
                out,
                "height",
                format_args!("{} ({depth} deep)", c.fetched.height),
            )?,
            None => row(out, "height", c.fetched.height)?,
        }
    }

    for (n, item) in disclosure.items().iter().enumerate() {
        row(
            out,
            format_args!("item {:<2}", n + 1),
            format_args!("{}  {}", output_name(item), kind_name(item.kind)),
        )?;
        sub_row(out, "pk_d", hex::encode(item.pk_d))?;
        sub_row(out, "secret", hex::encode(item.secret))?;
        let result = checked.and_then(|c| c.report.items.iter().find(|(i, _)| i == item));
        if let Some((_, result)) = result {
            print_result(out, params, result)?;
        }
    }
    for unknown in disclosure.unknown_items() {
        row(
            out,
            "skipped",
            format_args!(
                "typecode {:#04x}, body {}",
                unknown.typecode,
                hex::encode(&unknown.body)
            ),
        )?;
    }
    Ok(())
}

fn print_result(
    out: &mut dyn Write,
    params: &Params,
    result: &Result<DisclosedNote, ItemError>,
) -> io::Result<()> {
    let note = match result {
        Ok(note) => note,
        Err(e) => return sub_row(out, "result", format_args!("FAILED: {e}")),
    };
    sub_row(out, "result", "verified")?;
    sub_row(
        out,
        "value",
        format_args!("{} ZEC ({} zatoshis)", zec_amount(note.value), note.value),
    )?;
    sub_row(out, "address", receiver_address(params, note))?;
    sub_row(out, "memo", memo_text(&note.memo))?;
    sub_row(
        out,
        "receiver-decryptable",
        if note.receivable() { "yes" } else { "no" },
    )
}
