//! `--network` and the regtest activation heights.

use std::fs;
use std::path::PathBuf;

use clap::{Args, ValueEnum};
use zcash_protocol::consensus::Network;
use zdisclosure_core::network as core;

use crate::Error;

pub use zdisclosure_core::network::Params;

/// The network a tool runs against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Chain {
    /// Mainnet.
    Main,
    /// Testnet.
    Test,
    /// A local regtest chain.
    Regtest,
}

/// The network flags every tool takes.
#[derive(Args, Clone, Debug)]
pub struct NetworkArgs {
    /// The network the transaction and the Disclosure belong to.
    #[arg(long, value_enum, default_value_t = Chain::Main)]
    pub network: Chain,

    /// Regtest only: a file of `<upgrade>=<height|none>` lines. Every upgrade through NU6.3
    /// otherwise activates at height 1.
    #[arg(long, value_name = "FILE")]
    pub activation_heights: Option<PathBuf>,

    /// Regtest only: sets one activation height, as `<upgrade>=<height|none>`. Overrides the
    /// file. Can repeat.
    #[arg(long, value_name = "UPGRADE=HEIGHT")]
    pub activation: Vec<String>,
}

impl NetworkArgs {
    /// Builds the consensus parameters.
    pub fn params(&self) -> Result<Params, Error> {
        let regtest_flags = self.activation_heights.is_some() || !self.activation.is_empty();
        match self.network {
            Chain::Main | Chain::Test if regtest_flags => Err(Error::usage(
                "--activation-heights and --activation apply only to --network regtest",
            )),
            Chain::Main => Ok(Params::Public(Network::MainNetwork)),
            Chain::Test => Ok(Params::Public(Network::TestNetwork)),
            Chain::Regtest => {
                let mut net = core::default_regtest();
                if let Some(path) = &self.activation_heights {
                    let text = fs::read_to_string(path).map_err(|e| {
                        Error::usage(format!("cannot read {}: {e}", path.display()))
                    })?;
                    for (i, line) in text.lines().enumerate() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        core::set_activation(&mut net, line).map_err(|e| {
                            Error::usage(format!("{}:{}: {e}", path.display(), i + 1))
                        })?;
                    }
                }
                for entry in &self.activation {
                    core::set_activation(&mut net, entry)
                        .map_err(|e| Error::usage(format!("--activation {entry}: {e}")))?;
                }
                Ok(Params::Regtest(net))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zcash_protocol::consensus::BlockHeight;

    use super::*;

    fn args(network: Chain, file: Option<PathBuf>, activation: &[&str]) -> NetworkArgs {
        NetworkArgs {
            network,
            activation_heights: file,
            activation: activation.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn regtest_layers_apply_in_order() {
        let path = std::env::temp_dir().join(format!("zdisc-heights-{}", std::process::id()));
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "# comment\n\nnu5=10\nnu6.3 = 30").unwrap();

        let params = args(
            Chain::Regtest,
            Some(path.clone()),
            &["nu6.3=none", "nu7=40"],
        )
        .params()
        .unwrap();
        fs::remove_file(path).unwrap();

        let Params::Regtest(net) = params else {
            panic!("expected regtest")
        };
        assert_eq!(net.canopy, Some(BlockHeight::from_u32(1)));
        assert_eq!(net.nu5, Some(BlockHeight::from_u32(10)));
        assert_eq!(net.nu6_3, None);
        assert_eq!(net.nu7, Some(BlockHeight::from_u32(40)));
    }

    #[test]
    fn regtest_flags_need_regtest() {
        let err = args(Chain::Main, None, &["nu5=1"]).params().unwrap_err();
        assert_eq!(err.exit, crate::Exit::Usage);
        assert!(args(Chain::Regtest, None, &["nu9=1"]).params().is_err());
        assert!(args(Chain::Regtest, None, &["nu5"]).params().is_err());
    }
}
