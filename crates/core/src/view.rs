//! The fields a report shows: names, addresses and memos, as strings.

use zcash_disclosure::{DisclosedNote, Kind, Role};
use zcash_keys::address::Address;
use zcash_protocol::consensus::NetworkType;
use zcash_protocol::memo::{Memo, MemoBytes};
use zip32::Scope;

use crate::network::Params;

/// The network by its report name.
pub fn network_name(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Main => "main",
        NetworkType::Test => "test",
        NetworkType::Regtest => "regtest",
    }
}

/// The side of the payment an Item came from.
pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Receiver => "receiver",
        Role::Sender => "sender",
    }
}

/// The key scope an output was derived at. Internal is change.
pub fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::External => "external",
        Scope::Internal => "internal",
    }
}

/// The type of an Item, named for readers. `input::decode` refuses the kinds of the other
/// pools.
pub fn kind_name(kind: Kind) -> String {
    let name = match kind {
        Kind::Sapling => "Sapling",
        Kind::SaplingLegacySender => "Sapling before ZIP 212, sender",
        Kind::SaplingLegacyReceiver => "Sapling before ZIP 212, receiver",
        Kind::Orchard | Kind::Ironwood => "unsupported",
    };
    format!("type {:#04x} ({name})", kind.typecode())
}

/// The receiver as a Sapling address.
pub fn receiver_address(params: &Params, note: &DisclosedNote) -> String {
    let mut raw = [0; 43];
    raw[..11].copy_from_slice(&note.diversifier);
    raw[11..].copy_from_slice(&note.pk_d);
    sapling::PaymentAddress::from_bytes(&raw)
        .map(|a| Address::Sapling(Box::new(a)).encode(params))
        .unwrap_or_else(|| format!("raw {}", hex::encode(raw)))
}

/// The memo as text when it holds text, `empty` when it is empty, hex otherwise.
pub fn memo_text(bytes: &[u8; 512]) -> String {
    let parsed = MemoBytes::from_bytes(bytes)
        .ok()
        .and_then(|m| Memo::try_from(m).ok());
    match parsed {
        Some(Memo::Empty) => "empty".to_owned(),
        Some(Memo::Text(text)) => format!("{:?}", &*text),
        _ => {
            let end = bytes.iter().rposition(|&b| b != 0).map_or(1, |i| i + 1);
            format!("hex {}", hex::encode(&bytes[..end]))
        }
    }
}

/// Zatoshis as a ZEC amount, without trailing zeros in the fraction.
pub fn zec_amount(zatoshis: u64) -> String {
    let whole = zatoshis / 100_000_000;
    let frac = format!("{:08}", zatoshis % 100_000_000);
    match frac.trim_end_matches('0') {
        "" => whole.to_string(),
        frac => format!("{whole}.{frac}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{memo_text, zec_amount};

    #[test]
    fn amounts_drop_trailing_zeros() {
        assert_eq!(zec_amount(0), "0");
        assert_eq!(zec_amount(1), "0.00000001");
        assert_eq!(zec_amount(1_000_000), "0.01");
        assert_eq!(zec_amount(100_000_000), "1");
        assert_eq!(zec_amount(1_050_000_000), "10.5");
        assert_eq!(zec_amount(2_100_000_000_000_000), "21000000");
    }

    #[test]
    fn memos_render_by_kind() {
        let mut empty = [0; 512];
        empty[0] = 0xf6;
        assert_eq!(memo_text(&empty), "empty");

        let mut text = [0; 512];
        text[..5].copy_from_slice(b"Rent\n");
        assert_eq!(memo_text(&text), "\"Rent\\n\"");

        let mut arbitrary = [0; 512];
        arbitrary[..3].copy_from_slice(&[0xff, 0x01, 0x02]);
        assert_eq!(memo_text(&arbitrary), "hex ff0102");
    }
}
