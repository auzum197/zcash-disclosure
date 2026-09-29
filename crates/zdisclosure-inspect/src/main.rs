//! The `zdisclosure-inspect` binary. See the library for the interface.

use std::process::ExitCode;

use clap::Parser;
use zdisclosure_common::Io;
use zdisclosure_inspect::{Cli, run};

fn main() -> ExitCode {
    let cli = Cli::parse();
    Io::with_std(|io| run(cli, io)).into()
}
