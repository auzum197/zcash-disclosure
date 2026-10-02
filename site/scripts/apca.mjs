#!/usr/bin/env node
// APCA-W3 0.1.9 contrast audit for the site palette, replacing WCAG ratios.
// Every text pair targets Lc 75 (body minimum), display text 90. Placeholders and
// decorative glyphs target Lc 45.
// Run: npm run check:contrast
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const roots = [...css.matchAll(/:root\s*\{([^}]+)\}/g)].map((m) =>
  Object.fromEntries(
    [...m[1].matchAll(/--([\w-]+):\s*([^;]+);/g)].map((x) => [x[1], x[2].trim()]),
  ),
);
if (roots.length < 2) throw new Error("expected a dark and a light :root block");
const themes = { dark: roots[0], light: roots[1] };

const parse = (v) => {
  if (typeof v === "object") return v;
  v = v.trim();
  if (v[0] === "#") {
    const h = v.slice(1);
    const n = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
    const i = parseInt(n, 16);
    return { r: (i >> 16) & 255, g: (i >> 8) & 255, b: i & 255, a: 1 };
  }
  const m = v.match(/rgba?\(([^)]+)\)/);
  if (!m) throw new Error(`cannot parse color: ${v}`);
  const p = m[1].split(/[,\s/]+/).filter(Boolean).map(Number);
  return { r: p[0], g: p[1], b: p[2], a: p[3] ?? 1 };
};

const blend = (fg, bg) => ({
  r: fg.r * fg.a + bg.r * (1 - fg.a),
  g: fg.g * fg.a + bg.g * (1 - fg.a),
  b: fg.b * fg.a + bg.b * (1 - fg.a),
  a: 1,
});

const y = (c) => {
  const lin = (v) => (v / 255) ** 2.4;
  let Y = 0.2126729 * lin(c.r) + 0.7151522 * lin(c.g) + 0.072175 * lin(c.b);
  if (Y < 0.022) Y += (0.022 - Y) ** 1.414;
  return Y;
};

const Lc = (fg, bg) => {
  const Ytxt = y(parse(fg));
  const Ybg = y(parse(bg));
  if (Math.abs(Ybg - Ytxt) < 0.0005) return 0;
  let out;
  if (Ybg > Ytxt) {
    const sapc = (Ybg ** 0.56 - Ytxt ** 0.57) * 1.14;
    out = sapc < 0.1 ? 0 : sapc - 0.027;
  } else {
    const sapc = (Ybg ** 0.65 - Ytxt ** 0.62) * 1.14;
    out = sapc > -0.1 ? 0 : sapc + 0.027;
  }
  return Math.round(out * 1000) / 10;
};

const rows = [];
const check = (label, fg, bgs, min) =>
  rows.push({ label, min, lc: Math.min(...bgs.map((bg) => Math.abs(Lc(fg, bg)))) });

for (const [name, v] of Object.entries(themes)) {
  const bg = parse(v.bg);
  const card = [
    blend(parse(v["card-top"]), bg),
    blend(parse(v["card-bottom"]), bg),
  ];
  const well = [bg, ...card].map((c) => blend(parse(v.well), c));
  const tint = (colorVar, pct) => card.map((c) => blend({ ...parse(colorVar), a: pct }, c));
  const tintVar = (bgVar) => [bg, ...card].map((c) => blend(parse(v[bgVar]), c));

  check(`${name}  body text on page`, v.text, [bg], 90);
  check(`${name}  secondary copy on page`, v["text-2"], [bg], 75);
  check(`${name}  secondary copy on card`, v["text-2"], card, 75);
  check(`${name}  labels on page`, v["text-3"], [bg], 75);
  check(`${name}  labels on card`, v["text-3"], card, 75);
  check(`${name}  labels on well`, v["text-3"], well, 75);
  check(`${name}  placeholder in inputs`, v["text-4"], well, 45);
  check(`${name}  faint glyphs on page`, v["text-4"], [bg], 45);
  check(`${name}  button label on accent`, v["accent-ink"], [v.accent, v["accent-hi"]], 75);
  check(`${name}  highlighted string prefix`, v["accent-hi"], [bg], 75);
  check(`${name}  ok verdict text`, v.ok, tintVar("ok-bg"), 75);
  check(`${name}  fail verdict text`, v.bg, [v.text], 75);
  check(`${name}  error line text`, v.fail, tintVar("fail-bg"), 75);
  check(`${name}  warning line text`, v.warn, tintVar("warn-bg"), 75);
}

const labelWidth = Math.max(...rows.map((r) => r.label.length));
let failed = 0;
for (const r of rows) {
  const pass = r.lc >= r.min;
  if (!pass) failed += 1;
  console.log(
    `${pass ? "pass" : "FAIL"}  Lc ${String(r.lc).padStart(6)}  (min ${String(r.min).padStart(3)})  ${r.label.padEnd(labelWidth)}`,
  );
}
console.log(`\n${rows.length - failed}/${rows.length} pairs pass APCA`);
process.exitCode = failed ? 1 : 0;
