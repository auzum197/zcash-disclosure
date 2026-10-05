//! Prints the sample fixture the website ships: a regtest transaction, the two viewing
//! keys, and a Disclosure the receiver made for every external Sapling output.
//!
//! The builder is the one the end-to-end tests use, with a fixed seed, so the output is
//! reproducible. Run it as
//!
//! ```text
//! cargo run -p zdisclosure-verify --example make_fixture > site/src/data/sample.json
//! ```

use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BlockHeight;
use zdisclosure_core::network::{Params, default_regtest};
use zdisclosure_core::{produce, tx};
use zdisclosure_fixture::HEIGHT;

fn main() {
    let built = zdisclosure_fixture::build();
    let params = Params::Regtest(default_regtest());
    let height = BlockHeight::from_u32(HEIGHT);
    let parsed = tx::parse(&params, &built.raw, height).unwrap();
    let receiver = UnifiedFullViewingKey::decode(&params, built.receiver_ufvk.trim()).unwrap();
    let disclosure = disclose(&params, height, &parsed, &receiver);

    println!(
        "{}",
        serde_json::json!({
            "network": "regtest",
            "height": HEIGHT,
            "tx": hex::encode(built.raw),
            "receiverUfvk": built.receiver_ufvk,
            "senderUfvk": built.sender_ufvk,
            "disclosure": disclosure,
        })
    );
}

/// The Disclosure the receiver makes for every external output of `tx`.
fn disclose(
    params: &Params,
    height: BlockHeight,
    tx: &Transaction,
    key: &UnifiedFullViewingKey,
) -> String {
    let found = produce::discover(params, height, tx, key).unwrap();
    let chosen = produce::select(&found, true, false, &[]).unwrap();
    produce::build(params, tx, chosen.iter().map(|d| d.item.clone()).collect())
        .unwrap()
        .encode()
}
