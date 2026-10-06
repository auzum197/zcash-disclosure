# zdisclosure site

A SolidJS website over `zdisclosure-wasm`: inspect, verify and create
Disclosures in the browser.

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

## Link previews

Every route has its own title, description, canonical URL, Open Graph tags
and X card tags. They come from `src/meta.ts`. In dev, Vite writes them into
the head for the requested path. A build also writes `create.html`,
`inspect.html` and `verify.html` next to `index.html`, each with its own
head, because a crawler does not run the app. GitHub Pages serves `/create`
from `create.html`.

The images are 1200x630 PNGs in `public/og/`, one per route, with
`public/apple-touch-icon.png`. They are drawn from `scripts/og.html` and are
checked in. After a change to `src/meta.ts` or the template, draw them again:

```sh
pnpm og
```

To see the cards before a deploy, run `pnpm og:preview`. It starts the dev
server and opens `/og-preview.html`, which fetches each route as a crawler
does and shows four sections: the images and icons, a table of checks with a
column per route, the card as X, Slack, Discord, LinkedIn, Facebook and
iMessage lay it out for a chosen route, and the raw tags. Once the site is
live, the platforms' own tools show the real result: the Facebook Sharing Debugger, the LinkedIn
Post Inspector, and a draft post on X.

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
