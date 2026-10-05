//! Process plumbing for the `zdisclosure` tools: the standard streams, files, exit codes
//! and the transaction source. Everything that needs an operating system lives here, and
//! the logic it serves lives in `zdisclosure-core`.
//!
//! - [`input`] reads Disclosures and other input from arguments, files and stdin.
//! - [`source`] obtains a transaction and its height from a raw file or from lightwalletd.
//! - [`network`] turns `--network` and the regtest activation flags into consensus
//!   parameters.
//! - [`check`] wires the core's verification to a transaction source.

use std::fmt;
use std::io::{Read, Write};
use std::process::ExitCode;

pub mod check;
pub mod input;
pub mod network;
pub mod source;

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

    fn quiet(exit: Exit) -> Self {
        Self {
            exit,
            message: String::new(),
        }
    }

    /// Whether the error ends the run without a report, as a closed output pipe does.
    pub fn is_quiet(&self) -> bool {
        self.message.is_empty()
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

    /// Reports `result` on stderr when it failed, and returns its exit status. A quiet
    /// error is reported nowhere.
    pub fn finish(&mut self, program: &str, result: Result<Exit, Error>) -> Exit {
        result.unwrap_or_else(|e| {
            if !e.is_quiet() {
                self.warn(program, &e);
            }
            e.exit
        })
    }
}

/// Maps a failed write to stdout to a usage error. A closed output pipe ends the run
/// quietly with success instead, the way paged output does.
pub fn write_err(e: std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        Error::quiet(Exit::Ok)
    } else {
        Error::usage(format!("cannot write output: {e}"))
    }
}
