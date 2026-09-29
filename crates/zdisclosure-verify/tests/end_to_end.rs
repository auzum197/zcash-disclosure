//! End-to-end runs of the three tools against transactions built in-process on regtest.
//!
//! The builder uses mocked Sapling proofs and real Orchard-circuit proofs. No network is
//! involved, and the lightwalletd path is not covered here.

use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use incrementalmerkletree::frontier::CommitmentTree;
use incrementalmerkletree::witness::IncrementalWitness;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sapling::Rseed;
use transparent::builder::TransparentSigningSet;
use zcash_disclosure::{Disclosure, Kind};
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_primitives::transaction::builder::{BuildConfig, Builder, BundlePadding};
use zcash_primitives::transaction::fees::zip317;
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::local_consensus::LocalNetwork;
use zcash_protocol::memo::MemoBytes;
use zcash_protocol::value::Zatoshis;
use zdisclosure_common::network::default_regtest;
use zdisclosure_common::{Exit, Io};
use zip32::{AccountId, Scope};

const HEIGHT: u32 = 100_000;
const INPUT: u64 = 10_000_000;
const TO_SAPLING: u64 = 1_000_000;
const TO_ACTIONS: u64 = 2_000_000;

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("zdisc-e2e-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, contents: &str) -> String {
        let path = self.0.join(name);
        fs::write(&path, contents).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A transaction from a sender to a receiver, with the files the tools read.
struct Fixture {
    scratch: Scratch,
    tx: String,
    sender: String,
    receiver: String,
}

impl Fixture {
    /// Builds a transaction that spends a Sapling note of the sender and pays the receiver
    /// once in Sapling and once in the action pool the network's upgrades select: Ironwood
    /// under NU6.3, Orchard before it. The sender's change goes to its internal Sapling
    /// address.
    fn build(name: &str, params: LocalNetwork) -> Self {
        let sender = UnifiedSpendingKey::from_seed(&params, &[1; 32], AccountId::ZERO).unwrap();
        let receiver = UnifiedSpendingKey::from_seed(&params, &[2; 32], AccountId::ZERO).unwrap();
        let (sender_fvk, receiver_fvk) = (
            sender.to_unified_full_viewing_key(),
            receiver.to_unified_full_viewing_key(),
        );
        let s_dfvk = sender_fvk.sapling().unwrap();
        let s_orchard = sender_fvk.orchard().unwrap();

        let note = s_dfvk.default_address().1.create_note(
            sapling::value::NoteValue::from_raw(INPUT),
            Rseed::AfterZip212([7; 32]),
        );
        let mut tree = CommitmentTree::<sapling::Node, 32>::empty();
        tree.append(sapling::Node::from_cmu(&note.cmu())).unwrap();
        let witness = IncrementalWitness::from_tree(tree).unwrap();

        let ironwood = params.nu6_3.is_some();
        let builder = |change: u64| {
            let config = BuildConfig::Standard {
                sapling_anchor: Some(witness.root().into()),
                orchard_anchor: (!ironwood).then(orchard::Anchor::empty_tree),
                ironwood_anchor: ironwood.then(orchard::Anchor::empty_tree),
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
            b.add_sapling_output::<Infallible>(
                Some(s_dfvk.to_ovk(Scope::External)),
                receiver_fvk.sapling().unwrap().default_address().1,
                Zatoshis::from_u64(TO_SAPLING).unwrap(),
                memo("Invoice #2291"),
            )
            .unwrap();
            let to = receiver_fvk
                .orchard()
                .unwrap()
                .address_at(0u32, Scope::External);
            let ovk = Some(s_orchard.to_ovk(Scope::External));
            let value = Zatoshis::from_u64(TO_ACTIONS).unwrap();
            if ironwood {
                b.add_ironwood_output::<Infallible>(ovk, to, value, memo("Rent, October"))
            } else {
                b.add_orchard_output::<Infallible>(ovk, to, value, memo("Rent, October"))
            }
            .unwrap();
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
        let built = builder(INPUT - TO_SAPLING - TO_ACTIONS - fee)
            .mock_build(
                &TransparentSigningSet::new(),
                &[sender.sapling().clone()],
                &[],
                ChaCha8Rng::seed_from_u64(3),
            )
            .unwrap();
        let mut raw = vec![];
        built.transaction().write(&mut raw).unwrap();

        let scratch = Scratch::new(name);
        Fixture {
            tx: scratch.write("tx.hex", &format!("{}\n", hex::encode(raw))),
            sender: scratch.write("sender.ufvk", &sender_fvk.encode(&params)),
            receiver: scratch.write("receiver.ufvk", &receiver_fvk.encode(&params)),
            scratch,
        }
    }

    fn tx_args(&self) -> Vec<String> {
        [
            "--network",
            "regtest",
            "--tx",
            &self.tx,
            "--height",
            &HEIGHT.to_string(),
        ]
        .map(str::to_owned)
        .to_vec()
    }
}

fn memo(text: &str) -> MemoBytes {
    MemoBytes::from_bytes(text.as_bytes()).unwrap()
}

struct Run {
    exit: Exit,
    stdout: String,
    stderr: String,
}

fn run<C: Parser>(
    program: &str,
    args: &[String],
    stdin: &[u8],
    run: impl FnOnce(C, &mut Io<'_>) -> Exit,
) -> Run {
    let cli = C::try_parse_from(std::iter::once(program.to_owned()).chain(args.iter().cloned()))
        .unwrap_or_else(|e| panic!("{program} {args:?}: {e}"));
    let (mut stdin, mut stdout, mut stderr) = (stdin, vec![], vec![]);
    let exit = run(
        cli,
        &mut Io {
            stdin: &mut stdin,
            stdout: &mut stdout,
            stderr: &mut stderr,
        },
    );
    Run {
        exit,
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

fn create(args: &[String]) -> Run {
    run("zdisclosure-create", args, b"", zdisclosure_create::run)
}

fn verify(args: &[String], stdin: &[u8]) -> Run {
    run("zdisclosure-verify", args, stdin, zdisclosure_verify::run)
}

fn inspect(args: &[String]) -> Run {
    run("zdisclosure-inspect", args, b"", zdisclosure_inspect::run)
}

fn args(first: &[&str], rest: &[String]) -> Vec<String> {
    first
        .iter()
        .map(|s| s.to_string())
        .chain(rest.iter().cloned())
        .collect()
}

/// Returns `(pool:index, role, scope, value)` for each `--list` line.
fn listed(fixture: &Fixture, ufvk: &str) -> Vec<(String, String, String, u64)> {
    let r = create(&args(&["--ufvk-file", ufvk, "--list"], &fixture.tx_args()));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    r.stdout
        .lines()
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 6, "{line}");
            (
                format!("{}:{}", f[0], f[1]),
                f[2].into(),
                f[3].into(),
                f[4].parse().unwrap(),
            )
        })
        .collect()
}

#[test]
fn receiver_discloses_sapling_and_ironwood_outputs() {
    let fx = Fixture::build("ironwood", default_regtest());

    let mut outputs = listed(&fx, &fx.receiver);
    outputs.sort_by_key(|o| o.0.starts_with("sapling"));
    assert_eq!(outputs.len(), 2);
    assert!(outputs[0].0.starts_with("ironwood:"));
    assert!(outputs[1].0.starts_with("sapling:"));
    for (o, value) in outputs.iter().zip([TO_ACTIONS, TO_SAPLING]) {
        assert_eq!(
            (o.1.as_str(), o.2.as_str(), o.3),
            ("receiver", "external", value)
        );
    }

    let r = create(&args(
        &["--ufvk-file", &fx.receiver, "--all"],
        &fx.tx_args(),
    ));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    assert_eq!(r.stderr, "");
    let string = r.stdout.trim().to_owned();
    assert!(string.starts_with("zdiscregtest1"));
    let kinds: Vec<_> = Disclosure::decode(&string)
        .unwrap()
        .items()
        .iter()
        .map(|i| i.kind)
        .collect();
    assert_eq!(kinds, [Kind::Sapling, Kind::Ironwood]);

    let r = verify(&args(&[&string], &fx.tx_args()), b"");
    assert_eq!(
        (r.exit, r.stdout.as_str(), r.stderr.as_str()),
        (Exit::Ok, "", "")
    );

    let r = inspect(&args(&[&string], &fx.tx_args()));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    assert_eq!(r.stdout.matches("  result  verified").count(), 2);
    assert!(r.stdout.contains("0.01000000 ZEC (1000000 zatoshis)"));
    assert!(r.stdout.contains("\"Invoice #2291\""));
    assert!(r.stdout.contains("\"Rent, October\""));
    assert!(r.stdout.contains("address zregtestsapling1"));
    assert!(r.stdout.contains("address zuregtest1"), "{}", r.stdout);
    assert!(r.stdout.contains("does not show block inclusion"));

    let offline = inspect(&[string]);
    assert_eq!(offline.exit, Exit::Ok);
    assert!(offline.stdout.contains("network   regtest"));
    assert!(!offline.stdout.contains("result"));
}

#[test]
fn sender_needs_a_flag_for_change() {
    let fx = Fixture::build("change", default_regtest());

    let outputs = listed(&fx, &fx.sender);
    assert_eq!(outputs.len(), 3);
    let change = outputs.iter().find(|o| o.2 == "internal").unwrap();
    assert_eq!(
        outputs
            .iter()
            .filter(|o| o.1 == "sender" && o.2 == "external")
            .count(),
        2
    );

    let r = create(&args(&["--ufvk-file", &fx.sender, "--all"], &fx.tx_args()));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    assert_eq!(
        Disclosure::decode(r.stdout.trim()).unwrap().items().len(),
        2
    );

    let refused = create(&args(
        &["--ufvk-file", &fx.sender, "--output", &change.0],
        &fx.tx_args(),
    ));
    assert_eq!(refused.exit, Exit::Failed);
    assert!(refused.stderr.contains("--allow-internal"));

    let allowed = create(&args(
        &[
            "--ufvk-file",
            &fx.sender,
            "--output",
            &change.0,
            "--allow-internal",
        ],
        &fx.tx_args(),
    ));
    assert_eq!(allowed.exit, Exit::Ok, "{}", allowed.stderr);
    assert!(allowed.stderr.contains("links to every other change Item"));
    let r = verify(&args(&[allowed.stdout.trim()], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);

    let missing = create(&args(
        &["--ufvk-file", &fx.sender, "--output", "orchard:0"],
        &fx.tx_args(),
    ));
    assert_eq!(missing.exit, Exit::Failed);
}

#[test]
fn orchard_before_nu6_3() {
    let params = LocalNetwork {
        nu6_3: None,
        ..default_regtest()
    };
    let fx = Fixture::build("orchard", params);
    let tx_args = args(&["--activation", "nu6.3=none"], &fx.tx_args());

    let r = create(&args(&["--ufvk-file", &fx.receiver, "--all"], &tx_args));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    let d = Disclosure::decode(r.stdout.trim()).unwrap();
    assert_eq!(d.items()[1].kind, Kind::Orchard);

    let r = verify(&args(&[&d.encode()], &tx_args), b"");
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
}

#[test]
fn altered_or_misdirected_disclosures_fail() {
    let fx = Fixture::build("altered", default_regtest());
    let r = create(&args(
        &["--ufvk-file", &fx.receiver, "--all"],
        &fx.tx_args(),
    ));
    let good = Disclosure::decode(r.stdout.trim()).unwrap();

    let mut items = good.items().to_vec();
    items[0].secret[3] ^= 1;
    let altered = Disclosure::new(good.network(), good.txid(), items)
        .unwrap()
        .encode();
    let r = verify(&args(&[&altered], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Failed);
    assert_eq!(r.stderr.lines().count(), 1, "{}", r.stderr);
    assert!(r.stderr.contains("sapling:"));

    let quiet = verify(&args(&[&altered, "-q"], &fx.tx_args()), b"");
    assert_eq!((quiet.exit, quiet.stderr.as_str()), (Exit::Failed, ""));

    let r = inspect(&args(&[&altered], &fx.tx_args()));
    assert_eq!(r.exit, Exit::Failed);
    assert!(r.stdout.contains("FAILED"));

    let other_tx = zcash_protocol::TxId::from_bytes([9; 32]);
    let elsewhere = Disclosure::new(good.network(), other_tx, good.items().to_vec()).unwrap();
    let r = verify(&args(&[&elsewhere.encode()], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Failed);
    assert!(r.stderr.contains("does not match the txid"));
}

#[test]
fn a_set_file_collects_items_and_verifies_from_stdin() {
    let fx = Fixture::build("set", default_regtest());
    let outputs = listed(&fx, &fx.receiver);
    let set = fx.scratch.path("disclosures.zdisc");

    for (output, ..) in &outputs {
        let r = create(&args(
            &[
                "--ufvk-file",
                &fx.receiver,
                "--output",
                output,
                "--out",
                &set,
            ],
            &fx.tx_args(),
        ));
        assert_eq!((r.exit, r.stdout.as_str()), (Exit::Ok, ""), "{}", r.stderr);
    }
    let text = fs::read_to_string(&set).unwrap();
    let parsed = zcash_disclosure::set::parse(&text).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].items().len(), 2);

    let r = verify(&args(&["-"], &fx.tx_args()), text.as_bytes());
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    let r = verify(&args(&[&set], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
}

#[test]
fn qr_output_draws_blocks() {
    let fx = Fixture::build("qr", default_regtest());
    let r = create(&args(
        &["--ufvk-file", &fx.receiver, "--all", "--qr"],
        &fx.tx_args(),
    ));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    assert!(r.stdout.lines().count() > 20);
    assert!(r.stdout.contains('▀') || r.stdout.contains('▄') || r.stdout.contains('█'));
}

#[test]
fn usage_and_source_errors_have_their_codes() {
    let fx = Fixture::build("usage", default_regtest());

    let r = verify(&args(&["zdiscregtest1qqqq"], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Usage);

    let r = verify(&args(&["-", "--tx", "-", "--height", "1"], &[]), b"");
    assert_eq!(r.exit, Exit::Usage);

    let missing = fx.scratch.path("absent.hex");
    let r = create(&args(
        &["--ufvk-file", &fx.receiver, "--all", "--network", "regtest"],
        &[
            "--tx".into(),
            missing,
            "--height".into(),
            HEIGHT.to_string(),
        ],
    ));
    assert_eq!(r.exit, Exit::Usage);

    let r = create(&args(&["--ufvk-file", &fx.tx, "--all"], &fx.tx_args()));
    assert_eq!(r.exit, Exit::Usage);
    assert!(
        !r.stderr
            .contains(&fs::read_to_string(&fx.tx).unwrap()[..20])
    );
    assert!(Path::new(&fx.tx).exists());
}
