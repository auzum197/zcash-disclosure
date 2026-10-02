//! Consensus parameters for the three networks, and the regtest activation schedules.

use zcash_protocol::consensus::{BlockHeight, Network, NetworkType, NetworkUpgrade, Parameters};
use zcash_protocol::local_consensus::LocalNetwork;

/// Consensus parameters for any of the three networks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Params {
    /// Mainnet or testnet.
    Public(Network),
    /// Regtest with its activation heights.
    Regtest(LocalNetwork),
}

impl Parameters for Params {
    fn network_type(&self) -> NetworkType {
        match self {
            Params::Public(n) => n.network_type(),
            Params::Regtest(n) => n.network_type(),
        }
    }

    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        match self {
            Params::Public(n) => n.activation_height(nu),
            Params::Regtest(n) => n.activation_height(nu),
        }
    }
}

/// Regtest with every upgrade from Overwinter to NU6.3 at height 1 and NU7 unset.
pub fn default_regtest() -> LocalNetwork {
    let one = Some(BlockHeight::from_u32(1));
    LocalNetwork {
        overwinter: one,
        sapling: one,
        blossom: one,
        heartwood: one,
        canopy: one,
        nu5: one,
        nu6: one,
        nu6_1: one,
        nu6_2: one,
        nu6_3: one,
        nu7: None,
        #[cfg(zcash_unstable = "nutachyon")]
        nu_tachyon: None,
    }
}

/// Sets one activation height, as `<upgrade>=<height|none>`, on `net`.
pub fn set_activation(net: &mut LocalNetwork, entry: &str) -> Result<(), String> {
    let (name, height) = entry
        .split_once('=')
        .ok_or("expected <upgrade>=<height|none>")?;
    let height = match height.trim() {
        "none" => None,
        h => Some(BlockHeight::from_u32(
            h.parse().map_err(|_| format!("invalid height {h:?}"))?,
        )),
    };
    let slot = match name.trim() {
        "overwinter" => &mut net.overwinter,
        "sapling" => &mut net.sapling,
        "blossom" => &mut net.blossom,
        "heartwood" => &mut net.heartwood,
        "canopy" => &mut net.canopy,
        "nu5" => &mut net.nu5,
        "nu6" => &mut net.nu6,
        "nu6.1" => &mut net.nu6_1,
        "nu6.2" => &mut net.nu6_2,
        "nu6.3" => &mut net.nu6_3,
        "nu7" => &mut net.nu7,
        other => return Err(format!("unknown upgrade {other:?}")),
    };
    *slot = height;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_apply_in_order() {
        let net = match regtest(["nu6.3=none", "nu5=10", "nu6.3 = 30"]).unwrap() {
            Params::Regtest(net) => net,
            Params::Public(_) => panic!("expected regtest"),
        };
        assert_eq!(net.canopy, Some(BlockHeight::from_u32(1)));
        assert_eq!(net.nu5, Some(BlockHeight::from_u32(10)));
        assert_eq!(net.nu6_3, Some(BlockHeight::from_u32(30)));
        assert_eq!(net.nu7, None);
    }

    #[test]
    fn bad_entries_are_refused() {
        assert!(regtest(["nu9=1"]).is_err());
        assert!(regtest(["nu5"]).is_err());
        assert!(regtest(["nu5=x"]).is_err());
    }

    fn regtest<'a>(entries: impl IntoIterator<Item = &'a str>) -> Result<Params, String> {
        let mut net = default_regtest();
        for entry in entries {
            set_activation(&mut net, entry)?;
        }
        Ok(Params::Regtest(net))
    }
}
