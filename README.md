# zcash-disclosure-tools

Command-line tools for Zcash Sapling note Disclosures, built on the `zcash_disclosure` crate from the `feat/zcash-disclosure` branch of [auzum197/librustzcash](https://github.com/auzum197/librustzcash/pull/1).

A Disclosure is a `zdu1…` string that lets any holder decrypt and check chosen Sapling outputs of one transaction. It is unsigned. It says nothing about block inclusion, transaction validity, spend status or authorship. The tools refuse signed Disclosures (`zds1…`) and Items for Orchard or Ironwood outputs.

The format and the verification rules are in the draft specification, [zcash_disclosure/SPEC.md](https://github.com/auzum197/librustzcash/blob/feat/zcash-disclosure/zcash_disclosure/SPEC.md). It is a candidate for the disclosure encodings that [ZIP 311](https://zips.z.cash/zip-0311) leaves open.

| Binary | Purpose |
|---|---|
| `zdisclosure-create` | produces a Disclosure from a UFVK |
| `zdisclosure-verify` | checks a Disclosure and answers with its exit status |
| `zdisclosure-inspect` | prints a Disclosure and, given the transaction, what it opens |

```sh
# Sapling outputs the key can disclose.
zdisclosure-create --ufvk-file key.txt --lightwalletd https://zec.rocks:443 --txid <txid> --list

# A Disclosure for Sapling outputs 0 and 1.
zdisclosure-create --ufvk-file key.txt --lightwalletd https://zec.rocks:443 --txid <txid> \
    --output 0 --output 1 > disclosure.txt

# The same offline, from raw transaction bytes and the height of their block.
zdisclosure-create --ufvk-file key.txt --tx tx.hex --height 3500000 --all

zdisclosure-verify "$(cat disclosure.txt)" --lightwalletd https://zec.rocks:443 --min-confirmations 10
zdisclosure-inspect disclosure.txt --lightwalletd https://zec.rocks:443
```

## Building

```sh
cargo build --release
cargo test --workspace
```

The root `Cargo.toml` repeats librustzcash's `[patch.crates-io]` section. The `zcash_disclosure` crate depends on an `orchard` with Ironwood, and without the section the build takes crates.io `orchard`, which lacks it.

## License

MIT OR Apache-2.0.
