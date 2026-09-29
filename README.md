# zcash-disclosure-tools

Command-line tools for Zcash shielded note Disclosures, built on the `zcash_disclosure` crate from the `feat/zcash-disclosure` branch of [auzum197/librustzcash](https://github.com/auzum197/librustzcash/pull/1).

A Disclosure is a `zdisc1…` string that lets any holder decrypt and check chosen Sapling, Orchard or Ironwood outputs of one transaction. It is unsigned. It says nothing about block inclusion, transaction validity, spend status or authorship.

| Binary | Purpose |
|---|---|
| `zdisclosure-create` | produces a Disclosure from a UFVK |
| `zdisclosure-verify` | checks a Disclosure and answers with its exit status |
| `zdisclosure-inspect` | prints a Disclosure and, given the transaction, what it opens |

```sh
# Outputs the key can disclose.
zdisclosure-create --ufvk-file key.txt --lightwalletd https://zec.rocks:443 --txid <txid> --list

# A Disclosure for one Sapling output and one Ironwood output.
zdisclosure-create --ufvk-file key.txt --lightwalletd https://zec.rocks:443 --txid <txid> \
    --output sapling:0 --output ironwood:1 > disclosure.txt

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

The root `Cargo.toml` repeats librustzcash's `[patch.crates-io]` section. Without it the build takes crates.io `orchard`, which lacks Ironwood.

## License

MIT OR Apache-2.0.
