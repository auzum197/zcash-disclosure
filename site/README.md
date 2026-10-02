# zdisclosure site

A SolidJS website over `zdisclosure-wasm`: inspect, verify and create
Disclosures in the browser. Nothing is sent anywhere.

## Develop

```sh
pnpm install
pnpm dev
```

## Rebuild the wasm module

The glue in `src/wasm/` is generated and git-ignored. After any change to the
Rust crates, regenerate it:

```sh
pnpm wasm
```

The C part of the crypto stack needs a clang with a wasm backend. The script
picks up Homebrew LLVM when present, or set `CC_wasm32_unknown_unknown` and
`AR_wasm32_unknown_unknown` yourself.

## Check contrast

Contrast is measured with APCA-W3 0.1.9, not WCAG ratios. Every text pair
targets Lc 75, display text 90. Placeholders and decorative glyphs use the
faint tone and target Lc 45:

```sh
pnpm check:contrast
```

The script parses both theme blocks in `src/styles.css`, composites the
tinted surfaces, and fails on any pair under its level. Change colors there
and re-run it.

## Regenerate the sample fixture

The example on every page is a real regtest transaction with fixed seeds, so
it is reproducible:

```sh
cargo run -p zdisclosure-verify --example make_fixture > src/data/sample.json
```
