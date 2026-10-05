//! The sample Sapling transaction the end-to-end tests and the website fixture share:
//! a sender pays a receiver twice and takes change, with mocked proofs and fixed seeds,
//! so the bytes are the same on every build.

use std::convert::Infallible;

use incrementalmerkletree::frontier::CommitmentTree;
use incrementalmerkletree::witness::IncrementalWitness;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sapling::Rseed;
use transparent::builder::TransparentSigningSet;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_primitives::transaction::builder::{BuildConfig, Builder, BundlePadding};
use zcash_primitives::transaction::fees::zip317;
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::memo::MemoBytes;
use zcash_protocol::value::Zatoshis;
use zdisclosure_core::network::default_regtest;
use zip32::{AccountId, Scope};

/// The height the sample transaction is mined at.
pub const HEIGHT: u32 = 100_000;
/// The value the sender spends.
pub const INPUT: u64 = 10_000_000;
/// The first payment to the receiver.
pub const INVOICE: u64 = 1_000_000;
/// The second payment to the receiver.
pub const RENT: u64 = 2_000_000;

/// The sample transaction and the viewing keys that open it.
pub struct Built {
    /// The serialized transaction.
    pub raw: Vec<u8>,
    /// The sender's UFVK, encoded for regtest.
    pub sender_ufvk: String,
    /// The receiver's UFVK, encoded for regtest.
    pub receiver_ufvk: String,
}

/// Builds the sample transaction on regtest: the sender spends one note and pays the
/// receiver twice, and its change goes to the sender's internal Sapling address.
pub fn build() -> Built {
    let params = default_regtest();
    let sender = UnifiedSpendingKey::from_seed(&params, &[1; 32], AccountId::ZERO).unwrap();
    let receiver = UnifiedSpendingKey::from_seed(&params, &[2; 32], AccountId::ZERO).unwrap();
    let sender_fvk = sender.to_unified_full_viewing_key();
    let receiver_fvk = receiver.to_unified_full_viewing_key();
    let s_dfvk = sender_fvk.sapling().unwrap();
    let to = receiver_fvk.sapling().unwrap().default_address().1;

    let note = s_dfvk.default_address().1.create_note(
        sapling::value::NoteValue::from_raw(INPUT),
        Rseed::AfterZip212([7; 32]),
    );
    let mut tree = CommitmentTree::<sapling::Node, 32>::empty();
    tree.append(sapling::Node::from_cmu(&note.cmu())).unwrap();
    let witness = IncrementalWitness::from_tree(tree).unwrap();

    let builder = |change: u64| {
        let config = BuildConfig::Standard {
            sapling_anchor: Some(witness.root().into()),
            orchard_anchor: None,
            ironwood_anchor: None,
            orchard_padding: BundlePadding::DEFAULT,
            ironwood_padding: BundlePadding::DEFAULT,
        };
        let mut b = Builder::new(params, BlockHeight::from_u32(HEIGHT), config);
        b.add_sapling_spend::<Infallible>(
            s_dfvk.fvk().clone(),
            note.clone(),
            witness.path().unwrap(),
        )
        .unwrap();
        for (value, text) in [(INVOICE, "Invoice #2291"), (RENT, "Rent, October")] {
            b.add_sapling_output::<Infallible>(
                Some(s_dfvk.to_ovk(Scope::External)),
                to,
                Zatoshis::from_u64(value).unwrap(),
                MemoBytes::from_bytes(text.as_bytes()).unwrap(),
            )
            .unwrap();
        }
        b.add_sapling_output::<Infallible>(
            Some(s_dfvk.to_ovk(Scope::Internal)),
            s_dfvk.change_address().1,
            Zatoshis::from_u64(change).unwrap(),
            MemoBytes::empty(),
        )
        .unwrap();
        b
    };

    #[allow(deprecated)]
    let fee = u64::from(builder(0).get_fee(&zip317::FeeRule::standard()).unwrap());
    let built = builder(INPUT - INVOICE - RENT - fee)
        .mock_build(
            &TransparentSigningSet::new(),
            &[sender.sapling().clone()],
            &[],
            ChaCha8Rng::seed_from_u64(3),
        )
        .unwrap();
    let mut raw = vec![];
    built.transaction().write(&mut raw).unwrap();

    Built {
        raw,
        sender_ufvk: sender_fvk.encode(&params),
        receiver_ufvk: receiver_fvk.encode(&params),
    }
}
