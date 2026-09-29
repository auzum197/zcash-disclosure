//! Code shared by `zdisclosure-create`, `zdisclosure-verify` and `zdisclosure-inspect`.
//!
//! - [`network`] turns `--network` and the regtest activation flags into consensus
//!   parameters.
//! - [`source`] obtains a transaction and its height from a raw file or from lightwalletd.
//! - [`input`] reads Disclosures from an argument, stdin or a `.zdisc` file.

use std::fmt;
use std::io::{Read, Write};
use std::process::ExitCode;

use zcash_disclosure::{Item, Pool};

pub mod check;
pub mod input;
pub mod network;
pub mod source;

/// The name of a pool on the command line and in output.
pub fn pool_name(pool: Pool) -> &'static str {
    match pool {
        Pool::Sapling => "sapling",
        Pool::Orchard => "orchard",
        Pool::Ironwood => "ironwood",
    }
}

/// The output an Item points at, as `<pool>:<index>`.
pub fn output_name(item: &Item) -> String {
    format!("{}:{}", pool_name(item.kind.pool()), item.index)
}

/// The exit statuses the tools share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Everything checked out.
    Ok = 0,
    /// A Disclosure or an Item did not verify, or the requested outputs cannot be disclosed.
    Failed = 1,
    /// Bad arguments, unreadable input, or a string that breaks the canonical form.
    Usage = 2,
    /// The transaction could not be obtained.
    Unavailable = 3,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit as u8)
    }
}

/// An error that ends a run, with the status it ends it with.
#[derive(Debug)]
pub struct Error {
    /// The exit status.
    pub exit: Exit,
    message: String,
}

impl Error {
    /// A usage or input error.
    pub fn usage(message: impl fmt::Display) -> Self {
        Self::new(Exit::Usage, message)
    }

    /// A failed check.
    pub fn failed(message: impl fmt::Display) -> Self {
        Self::new(Exit::Failed, message)
    }

    /// A transaction that could not be obtained.
    pub fn unavailable(message: impl fmt::Display) -> Self {
        Self::new(Exit::Unavailable, message)
    }

    fn new(exit: Exit, message: impl fmt::Display) -> Self {
        Self {
            exit,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// The standard streams of a run. Tests pass buffers.
pub struct Io<'a> {
    /// Standard input.
    pub stdin: &'a mut dyn Read,
    /// Standard output.
    pub stdout: &'a mut dyn Write,
    /// Standard error.
    pub stderr: &'a mut dyn Write,
}

impl Io<'_> {
    /// Runs `f` against the process's own streams.
    pub fn with_std<T>(f: impl FnOnce(&mut Io<'_>) -> T) -> T {
        let (mut stdin, mut stdout, mut stderr) = (
            std::io::stdin().lock(),
            std::io::stdout().lock(),
            std::io::stderr(),
        );
        f(&mut Io {
            stdin: &mut stdin,
            stdout: &mut stdout,
            stderr: &mut stderr,
        })
    }

    /// Writes `message` to stderr, prefixed with the program name.
    pub fn warn(&mut self, program: &str, message: impl fmt::Display) {
        // A closed stderr leaves nowhere to report to.
        let _ = writeln!(self.stderr, "{program}: {message}");
    }

    /// Reports `result` on stderr when it failed, and returns its exit status.
    pub fn finish(&mut self, program: &str, result: Result<Exit, Error>) -> Exit {
        result.unwrap_or_else(|e| {
            self.warn(program, &e);
            e.exit
        })
    }
}

/// Maps a failed write to stdout to a usage error.
pub fn write_err(e: std::io::Error) -> Error {
    Error::usage(format!("cannot write output: {e}"))
}
