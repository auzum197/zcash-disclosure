//! Disclosures from strings: the HRP prefix rule, decoding and the canonical form.

use zcash_disclosure::{Class, Disclosure, Pool, set};
use zcash_protocol::consensus::NetworkType;

use crate::Error;

/// Decodes one Disclosure string and enforces the whole canonical form.
pub fn decode(s: &str) -> Result<Disclosure, Error> {
    let disclosure = Disclosure::decode(s).map_err(|e| Error::InvalidDisclosure(e.to_string()))?;
    covered(&disclosure)?;
    Ok(disclosure)
}

/// Rejects a Disclosure the tools do not handle: the signed class, an Item outside the
/// Sapling pool, and unknown Items out of order.
pub fn covered(disclosure: &Disclosure) -> Result<(), Error> {
    if disclosure.class() == Class::Signed {
        return Err(Error::SignedClass);
    }
    if let Some(item) = disclosure
        .items()
        .iter()
        .find(|item| item.kind.pool() != Pool::Sapling)
    {
        return Err(Error::ForeignPool(item.kind.typecode()));
    }
    sorted(disclosure)
}

/// Rejects unknown Items that repeat a typecode. The decoder's sort check compares them
/// with `<=`, while the canonical form is a strict increase.
fn sorted(disclosure: &Disclosure) -> Result<(), Error> {
    if disclosure
        .unknown_items()
        .windows(2)
        .all(|w| w[0].typecode < w[1].typecode)
    {
        Ok(())
    } else {
        Err(Error::UnsortedItems)
    }
}

/// Parses text that holds either one Disclosure or a Disclosure set.
pub fn parse_text(text: &str) -> Result<Vec<Disclosure>, Error> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    match (lines.next(), lines.next()) {
        (None, _) => Err(Error::EmptyInput),
        (Some(line), None) => decode(line).map(|d| vec![d]),
        _ => {
            let parsed = set::parse(text).map_err(|e| Error::InvalidSet(e.to_string()))?;
            parsed.iter().try_for_each(covered)?;
            Ok(parsed)
        }
    }
}

/// Whether `arg` names a Disclosure string rather than a path: it starts with an HRP of
/// either class followed by `1`.
pub fn is_disclosure_string(arg: &str) -> bool {
    [Class::Unsigned, Class::Signed].iter().any(|&class| {
        [NetworkType::Main, NetworkType::Test, NetworkType::Regtest]
            .iter()
            .any(|&n| arg.starts_with(&format!("{}1", zcash_disclosure::hrp(class, n))))
    })
}

#[cfg(test)]
mod tests {
    use zcash_disclosure::{Item, Kind};
    use zcash_protocol::TxId;

    use super::*;

    fn disclosure(txid: u8) -> Disclosure {
        Disclosure::new(
            NetworkType::Test,
            TxId::from_bytes([txid; 32]),
            vec![Item {
                kind: Kind::Sapling,
                index: 3,
                pk_d: [7; 32],
                secret: [9; 32],
            }],
        )
        .unwrap()
    }

    #[test]
    fn whitespace_around_one_disclosure_is_not_canonical() {
        let padded = format!("  {}  \n", disclosure(1).encode());
        assert!(decode(&padded).is_err());
        let padded = format!("{}  \n", disclosure(1).encode());
        assert!(parse_text(&padded).is_err());
    }

    /// Bech32m without the 90-character limit, as the encoder uses it.
    struct Unbounded {}

    impl bech32::Checksum for Unbounded {
        type MidstateRepr = <bech32::Bech32m as bech32::Checksum>::MidstateRepr;
        const CODE_LENGTH: usize = 4194368;
        const CHECKSUM_LENGTH: usize = bech32::Bech32m::CHECKSUM_LENGTH;
        const GENERATOR_SH: [u32; 5] = bech32::Bech32m::GENERATOR_SH;
        const TARGET_RESIDUE: u32 = bech32::Bech32m::TARGET_RESIDUE;
    }

    fn encode(hrp: &str, items: &[u8]) -> String {
        let mut payload = vec![zcash_disclosure::VERSION];
        payload.extend_from_slice(disclosure(1).txid().as_ref());
        payload.extend_from_slice(items);
        bech32::encode::<Unbounded>(bech32::Hrp::parse_unchecked(hrp), &payload).unwrap()
    }

    fn with_unknown_items(count: usize) -> String {
        let item = disclosure(1).items()[0].clone();
        let mut items = vec![0x02, 65, 3];
        items.extend_from_slice(&item.pk_d);
        items.extend_from_slice(&item.secret);
        for _ in 0..count {
            items.extend_from_slice(&[0x20, 2, 0xaa, 0xbb]);
        }
        encode("zdutest", &items)
    }

    #[test]
    fn unknown_items_may_not_repeat_a_typecode() {
        let ok = decode(&with_unknown_items(1)).unwrap();
        assert_eq!(ok.unknown_items().len(), 1);

        let err = decode(&with_unknown_items(2)).unwrap_err();
        assert!(err.to_string().contains("not sorted"));

        let set = with_unknown_items(1);
        let text = format!("{set}\n{}\n", disclosure(2).encode());
        assert!(parse_text(&text).is_ok());
    }

    #[test]
    fn other_pools_and_the_signed_class_are_refused() {
        for kind in [Kind::Orchard, Kind::Ironwood] {
            let mut item = disclosure(1).items()[0].clone();
            item.kind = kind;
            let d = Disclosure::new(NetworkType::Test, disclosure(1).txid(), vec![item]).unwrap();
            let err = decode(&d.encode()).unwrap_err();
            assert!(err.to_string().contains("not a Sapling item"), "{err}");
            let text = format!("{}\n{}\n", disclosure(2).encode(), d.encode());
            assert!(parse_text(&text).is_err());
        }

        let mut signer = vec![0xe0, 0xfd, 0x41, 0x01];
        signer.extend_from_slice(&[0; 321]);
        let signed = encode("zdstest", &signer);
        assert!(is_disclosure_string(&signed));
        let err = decode(&signed).unwrap_err();
        assert!(err.to_string().contains("signed class"), "{err}");
    }
}
