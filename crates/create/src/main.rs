//! The `zdisclosure-create` binary. See the library for the interface.

use std::process::ExitCode;

use clap::Parser;
use zdisclosure_cli::Io;
use zdisclosure_create::{Cli, run};

fn main() -> ExitCode {
    let cli = Cli::parse();
    Io::with_std(|io| run(cli, io)).into()
}
