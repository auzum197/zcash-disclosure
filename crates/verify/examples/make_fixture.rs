//! Prints the sample fixture the website ships: a regtest transaction, the two viewing
//! keys, and a Disclosure the receiver made for every external Sapling output.
//!
//! The builder is the one the end-to-end tests use, with a fixed seed, so the output is
//! reproducible. Run it as
//!
//! ```text
//! cargo run -p zdisclosure-verify --example make_fixture > site/src/data/sample.json
//! ```

use std::convert::Infallible;

use incrementalmerkletree::frontier::CommitmentTree;
use incrementalmerkletree::witness::IncrementalWitness;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sapling::Rseed;
use transparent::builder::TransparentSigningSet;
use zcash_disclosure::Disclosable;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_primitives::transaction::builder::{BuildConfig, Builder, BundlePadding};
use zcash_primitives::transaction::fees::zip317;
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::memo::MemoBytes;
use zcash_protocol::value::Zatoshis;
use zdisclosure_core::network::{Params, default_regtest};
use zip32::{AccountId, Scope};

const HEIGHT: u32 = 100_000;
const INPUT: u64 = 10_000_000;
const INVOICE: u64 = 1_000_000;
const RENT: u64 = 2_000_000;

fn main() {
    let params = Params::Regtest(default_regtest());
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

    let receiver_key = receiver.to_unified_full_viewing_key();
    let disclosure = disclose(&params, HEIGHT, built.transaction(), &receiver_key);

    println!(
        "{}",
        serde_json::json!({
            "network": "regtest",
            "height": HEIGHT,
            "tx": hex::encode(raw),
            "receiverUfvk": receiver_fvk.encode(&params),
            "senderUfvk": sender_fvk.encode(&params),
            "disclosure": disclosure,
        })
    );
}

/// The Disclosure the receiver makes for every external output of `tx`.
fn disclose(
    params: &Params,
    height: u32,
    tx: &zcash_primitives::transaction::Transaction,
    key: &zcash_keys::keys::UnifiedFullViewingKey,
) -> String {
    let found: Vec<Disclosable> =
        zdisclosure_core::produce::discover(params, height.into(), tx, key).unwrap();
    let chosen = zdisclosure_core::produce::select(&found, true, false, &[]).unwrap();
    zdisclosure_core::produce::build(params, tx, chosen.iter().map(|d| d.item.clone()).collect())
        .unwrap()
        .encode()
}
