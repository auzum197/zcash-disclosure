//! End-to-end runs of the three tools against transactions built in-process on regtest.
//!
//! The transactions are Sapling-only and the builder mocks their proofs. No network is
//! involved, and the lightwalletd path is not covered here.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use zcash_disclosure::{Disclosure, Kind};
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::{BlockHeight, BranchId};
use zdisclosure_cli::{Exit, Io};
use zdisclosure_core::network::default_regtest;
use zdisclosure_fixture::{HEIGHT, INVOICE, RENT};
use zip32::AccountId;

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

/// The sample transaction from `zdisclosure_fixture`, with the files the tools read.
struct Fixture {
    scratch: Scratch,
    tx: String,
    sender: String,
    receiver: String,
}

impl Fixture {
    fn build(name: &str) -> Self {
        let built = zdisclosure_fixture::build();
        let scratch = Scratch::new(name);
        Fixture {
            tx: scratch.write("tx.hex", &format!("{}\n", hex::encode(built.raw))),
            sender: scratch.write("sender.ufvk", &built.sender_ufvk),
            receiver: scratch.write("receiver.ufvk", &built.receiver_ufvk),
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

/// Returns `(index, role, scope, value)` for each `--list` line.
fn listed(fixture: &Fixture, ufvk: &str) -> Vec<(String, String, String, u64)> {
    let r = create(&args(&["--ufvk-file", ufvk, "--list"], &fixture.tx_args()));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    r.stdout
        .lines()
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 5, "{line}");
            assert!(f[0].parse::<u32>().is_ok(), "{line}");
            (f[0].into(), f[1].into(), f[2].into(), f[3].parse().unwrap())
        })
        .collect()
}

#[test]
fn receiver_discloses_its_sapling_outputs() {
    let fx = Fixture::build("receiver");

    let mut outputs = listed(&fx, &fx.receiver);
    outputs.sort_by_key(|o| o.3);
    assert_eq!(outputs.len(), 2);
    for (o, value) in outputs.iter().zip([INVOICE, RENT]) {
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
    assert!(string.starts_with("zduregtest1"));
    let kinds: Vec<_> = Disclosure::decode(&string)
        .unwrap()
        .items()
        .iter()
        .map(|i| i.kind)
        .collect();
    assert_eq!(kinds, [Kind::Sapling, Kind::Sapling]);

    let r = verify(&args(&[&string], &fx.tx_args()), b"");
    assert_eq!(
        (r.exit, r.stdout.as_str(), r.stderr.as_str()),
        (Exit::Ok, "", "")
    );

    let r = inspect(&args(&[&string], &fx.tx_args()));
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);
    assert_eq!(
        r.stdout.matches("  result               verified").count(),
        2
    );
    assert!(r.stdout.contains("0.01 ZEC (1000000 zatoshis)"));
    assert!(r.stdout.contains("0.02 ZEC (2000000 zatoshis)"));
    assert!(r.stdout.contains("\"Invoice #2291\""));
    assert!(r.stdout.contains("\"Rent, October\""));
    assert_eq!(
        r.stdout
            .matches("  address              zregtestsapling1")
            .count(),
        2,
        "{}",
        r.stdout
    );
    assert_eq!(r.stdout.matches("  receiver-decryptable yes").count(), 2);

    let offline = inspect(&[string]);
    assert_eq!(offline.exit, Exit::Ok);
    assert!(offline.stdout.contains("network                regtest"));
    assert!(!offline.stdout.contains("result"));
}

#[test]
fn sender_needs_a_flag_for_change() {
    let fx = Fixture::build("change");

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
    assert_eq!(allowed.stderr, "");
    let r = verify(&args(&[allowed.stdout.trim()], &fx.tx_args()), b"");
    assert_eq!(r.exit, Exit::Ok, "{}", r.stderr);

    let missing = create(&args(
        &["--ufvk-file", &fx.sender, "--output", "9"],
        &fx.tx_args(),
    ));
    assert_eq!(missing.exit, Exit::Failed);
}

#[test]
fn items_of_other_pools_are_refused() {
    let fx = Fixture::build("pools");
    let r = create(&args(
        &["--ufvk-file", &fx.receiver, "--all"],
        &fx.tx_args(),
    ));
    let good = Disclosure::decode(r.stdout.trim()).unwrap();

    for kind in [Kind::Orchard, Kind::Ironwood] {
        let mut items = good.items().to_vec();
        items[1].kind = kind;
        let mixed = Disclosure::new(good.network(), good.txid(), items)
            .unwrap()
            .encode();

        let r = verify(&args(&[&mixed], &fx.tx_args()), b"");
        assert_eq!(r.exit, Exit::Usage);
        assert!(r.stderr.contains("not a Sapling item"), "{}", r.stderr);
        assert_eq!(inspect(&[mixed]).exit, Exit::Usage);
    }
}

/// A v5 transaction with one transparent output and no shielded bundle, serialized by hand.
fn transparent_only_tx() -> Vec<u8> {
    let branch = BranchId::for_height(&default_regtest(), BlockHeight::from_u32(HEIGHT));
    let mut raw = vec![];
    raw.extend_from_slice(&0x8000_0005u32.to_le_bytes());
    raw.extend_from_slice(&0x26A7_270Au32.to_le_bytes());
    raw.extend_from_slice(&u32::from(branch).to_le_bytes());
    raw.extend_from_slice(&[0; 8]);
    raw.push(0);
    raw.push(1);
    raw.extend_from_slice(&1_000u64.to_le_bytes());
    raw.push(25);
    raw.extend_from_slice(&[0x76, 0xa9, 0x14]);
    raw.extend_from_slice(&[0x11; 20]);
    raw.extend_from_slice(&[0x88, 0xac]);
    raw.extend_from_slice(&[0, 0, 0]);
    raw
}

#[test]
fn a_transaction_without_sapling_outputs_is_unsupported() {
    let fx = Fixture::build("no-sapling");
    let tx = fx
        .scratch
        .write("transparent.hex", &hex::encode(transparent_only_tx()));
    for flags in [&["--list"][..], &["--all"], &["--output", "0"]] {
        let r = create(&args(
            &[
                "--ufvk-file",
                &fx.receiver,
                "--network",
                "regtest",
                "--tx",
                &tx,
                "--height",
            ],
            &[HEIGHT.to_string()]
                .into_iter()
                .chain(flags.iter().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
        ));
        assert_eq!(r.exit, Exit::Usage, "{flags:?}: {}", r.stderr);
        assert!(r.stdout.is_empty(), "{flags:?}");
        assert!(
            r.stderr.contains("it has no Sapling output"),
            "{flags:?}: {}",
            r.stderr
        );
    }
}

#[test]
fn a_key_that_opens_no_output_fails() {
    let fx = Fixture::build("stranger");
    let params = default_regtest();
    let stranger = UnifiedSpendingKey::from_seed(&params, &[3; 32], AccountId::ZERO)
        .unwrap()
        .to_unified_full_viewing_key()
        .encode(&params);
    let key = fx.scratch.write("stranger.ufvk", &stranger);
    for flags in [&["--list"][..], &["--all"], &["--output", "0"]] {
        let r = create(&args(
            &["--ufvk-file", &key],
            &fx.tx_args()
                .into_iter()
                .chain(flags.iter().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
        ));
        assert_eq!(r.exit, Exit::Failed, "{flags:?}: {}", r.stderr);
        assert!(r.stdout.is_empty(), "{flags:?}");
        assert!(
            r.stderr.contains("the key opens no output to disclose"),
            "{flags:?}: {}",
            r.stderr
        );
    }
}

#[test]
fn altered_or_misdirected_disclosures_fail() {
    let fx = Fixture::build("altered");
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
    assert!(r.stderr.contains(" output "));

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
    let fx = Fixture::build("set");
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
    let fx = Fixture::build("qr");
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
    let fx = Fixture::build("usage");

    let r = verify(&args(&["zduregtest1qqqq"], &fx.tx_args()), b"");
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
