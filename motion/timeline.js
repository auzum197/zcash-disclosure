// the score: 124 BPM, four beats to the bar, seventeen bars. Every cue is in
// beats, and render(t) draws the frame for a time in seconds from nothing but
// t, so a given time always shows the same frame.
export const BPM = 124;
export const BEAT = 60 / BPM;
export const LENGTH = 68 * BEAT;

// the sign powers on at beat 2, the sequencer plays from bar 4 to bar 14,
// the buzz sounds only while a sign is on screen, and at beat 66 the picture
// and the sound cut to black together
export const IGNITE = 2;
export const SEQ = [16, 56];
export const BUZZ = [[IGNITE, 16], [56, 66]];
export const OFF = 66;

const IGNITE_SECONDS = 1.4;

function mulberry32(seed) {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const rng = mulberry32(2291);
const rand = (lo, hi) => lo + rng() * (hi - lo);

// the three faults from the site's Home page, seeded. A frame with hold set
// keeps its level until the next frame, a frame without it ramps linearly.
function stutter() {
  const cuts = Math.round(rand(4, 9));
  const frames = [{ opacity: 1, offset: 0, hold: true }];
  for (let i = 1; i < cuts; i++) {
    frames.push({
      opacity: i % 2 ? rand(0.04, 0.55) : rand(0.65, 1),
      offset: i / cuts,
      hold: true,
    });
  }
  frames.push({ opacity: 1, offset: 1 });
  return frames;
}

function dropout() {
  const dark = rand(0.03, 0.12);
  const hold = rand(0.2, 0.55);
  return [
    { opacity: 1, offset: 0, hold: true },
    { opacity: dark, offset: 0.08, hold: true },
    { opacity: dark, offset: 0.08 + hold, hold: true },
    { opacity: rand(0.5, 0.85), offset: 0.16 + hold, hold: true },
    { opacity: dark, offset: 0.2 + hold, hold: true },
    { opacity: 1, offset: 1 },
  ];
}

function sag() {
  const floor = rand(0.45, 0.7);
  const steps = 14;
  const frames = [{ opacity: 1, offset: 0 }];
  for (let i = 1; i < steps; i++) {
    frames.push({ opacity: floor + rand(-0.1, 0.1), offset: i / steps, hold: true });
  }
  frames.push({ opacity: 1, offset: 1 });
  return frames;
}

// [shape, start in beats, length in seconds]
const PLAN = [
  [stutter, 6, 0.5],
  [dropout, 8, 0.8],
  [stutter, 9.75, 0.32],
  [sag, 10.25, 0.8],
  [stutter, 13, 0.4],
  [dropout, 14.25, 0.7],
  [dropout, 63, 0.7],
];

export const FAULTS = PLAN.map(([shape, beat, dur]) => ({ at: beat * BEAT, dur, frames: shape() }));

function level(frames, p) {
  let i = frames.length - 2;
  while (i > 0 && frames[i].offset > p) i--;
  const a = frames[i];
  const b = frames[i + 1];
  if (a.hold) return a.opacity;
  return a.opacity + ((b.opacity - a.opacity) * (p - a.offset)) / (b.offset - a.offset);
}

function tube(t) {
  const fault = FAULTS.find((f) => t >= f.at && t < f.at + f.dur);
  return fault ? level(fault.frames, (t - fault.at) / fault.dur) : 1;
}

function bezier(x1, y1, x2, y2) {
  const at = (a, b, s) => 3 * a * s * (1 - s) ** 2 + 3 * b * s * s * (1 - s) + s ** 3;
  return (x) => {
    let s = x;
    for (let i = 0; i < 8; i++) {
      const dx = 3 * x1 * (1 - s) ** 2 + 6 * (x2 - x1) * s * (1 - s) + 3 * (1 - x2) * s * s;
      if (Math.abs(dx) < 1e-6) break;
      s = Math.min(1, Math.max(0, s - (at(x1, x2, s) - x) / dx));
    }
    return at(y1, y2, s);
  };
}

// the site's --ease-out, for the sign powering on
const easeOut = bezier(0.23, 1, 0.32, 1);

// Apple's spring, set by a damping ratio and a response in seconds, going
// from rest at 0 to 1. Damping 1 settles without overshoot, under 1 it
// overshoots a little. There is no duration, the settle time follows.
function spring(damping, response) {
  const w = (2 * Math.PI) / response;
  if (damping >= 1) return (s) => (s <= 0 ? 0 : 1 - Math.exp(-w * s) * (1 + w * s));
  const wd = w * Math.sqrt(1 - damping ** 2);
  const k = (damping * w) / wd;
  return (s) => (s <= 0 ? 0 : 1 - Math.exp(-damping * w * s) * (Math.cos(wd * s) + k * Math.sin(wd * s)));
}

// [position spring, rotation spring]. Text and cards move critically damped.
// The micrographics turn on Apple's rotation spring, damping 0.8.
const ENTER = [spring(1, 0.45), spring(1, 0.45)];
const EXIT = [spring(1, 0.3), spring(1, 0.3)];
const FOLD = [spring(1, 0.5), spring(0.8, 0.5)];
const GLIDE = [spring(1, 0.7), spring(0.8, 0.7)];

// the same rotation spring, started away from its target and already moving,
// so a constant spin hands its speed over to the settle without a stall
function settle(damping, response, x0, v0) {
  const w = (2 * Math.PI) / response;
  const wd = w * Math.sqrt(1 - damping ** 2);
  const b = (v0 + damping * w * x0) / wd;
  return (s) => Math.exp(-damping * w * s) * (x0 * Math.cos(wd * s) + b * Math.sin(wd * s));
}

// from the unlit --text-4 to --text, with the three halo layers coming up
// from transparent, as the site's ignite keyframes do
function ignite(t) {
  const p = Math.min(1, Math.max(0, (t - IGNITE * BEAT) / IGNITE_SECONDS));
  const e = easeOut(p);
  const mix = (a, b) => Math.round(a + (b - a) * e);
  const glow = (alpha) => `rgba(245, 244, 240, ${(alpha * e).toFixed(4)})`;
  return {
    color: `rgb(${mix(0xa0, 0xf5)}, ${mix(0x9f, 0xf4)}, ${mix(0x9a, 0xf0)})`,
    textShadow: `0 0 1px ${glow(0.6)}, 0 0 10px ${glow(0.2)}, 0 0 32px ${glow(0.08)}`,
  };
}

// a pose is a transform in pieces, so two poses can be mixed. The rotations
// apply before the translateZ, which moves along the element's own normal:
// that is what stacks the slabs. Nothing has a perspective, so the 3D is
// drawn orthographic and ISO is the true isometric view of a plane.
const REST = { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, s: 1, o: 1 };
const TURNS = new Set(["rx", "ry", "rz"]);
const ISO = { rx: 54.7356, rz: -45 };
const GLOBE_SCALE = 4.2;

const css = (p) =>
  `translate3d(${p.x}px, ${p.y}px, 0) rotateX(${p.rx}deg) rotateY(${p.ry}deg) rotateZ(${p.rz}deg) translateZ(${p.z}px) scale(${p.s})`;

// the micrographics. A track is a selector and its keys, [beat, pose,
// springs]. The first key is where the element starts.
const TRACKS = [
  // three outputs lie flat, then fold into a shielded stack
  ...[0, 1, 2].map((i) => [
    `.outputs .slab:nth-child(${i + 1})`,
    [[0, { x: (i - 1) * 170 }], [17 + i * 0.25, { ...ISO, y: 30, z: i * 30 }, FOLD]],
  ]),
  // the top output of a stack lifts off and lies flat: it is opened
  ...[0, 1].map((i) => [`.reveal .slab:nth-child(${i + 1})`, [[0, { ...ISO, z: i * 18 }]]]),
  [
    ".reveal .slab:nth-child(3)",
    [[0, { ...ISO, z: 36 }], [44.5, { ...ISO, z: 80 }, FOLD], [45.25, { y: -64 }, FOLD]],
  ],
  // the globe spins in the middle of the frame, then glides into the icon
  [".emblem", [[0, { x: 472, y: 109, s: GLOBE_SCALE }], [58, {}, GLIDE]]],
];

const RISE = { y: 24, o: 0 };
const STAGGER = 0.06;
// the globe spins at a constant 120 degrees a second for two beats, then
// settles face on. Its grid, axis and orbit fade as it settles, and what is
// left is exactly the icon.
const GLOBE = { spin: 56, settle: 58, yaw: -400, speed: 120, tilt: -24 };
const GLOBE_YAW = settle(0.8, 0.7, GLOBE.yaw + GLOBE.speed * (GLOBE.settle - GLOBE.spin) * BEAT, GLOBE.speed);
const GLOBE_TILT = settle(0.8, 0.7, GLOBE.tilt, 0);

function globe(t, reduce) {
  const t1 = GLOBE.settle * BEAT;
  if (t < t1) {
    return { yaw: GLOBE.yaw + GLOBE.speed * Math.max(0, t - GLOBE.spin * BEAT), tilt: GLOBE.tilt, detail: 1, scale: GLOBE_SCALE };
  }
  if (reduce) return { yaw: 0, tilt: 0, detail: 0, scale: 1 };
  const s = t - t1;
  return { yaw: GLOBE_YAW(s), tilt: GLOBE_TILT(s), detail: Math.max(0, 1 - GLIDE[1](s)), scale: GLOBE_SCALE + (1 - GLOBE_SCALE) * GLIDE[0](s) };
}

// the icon is a globe seen face on, radius 21 about (32, 32): a rim, a prime
// meridian, meridians 45 degrees either side, an equator and parallels at 30
// degrees. Projected at yaw 0 and tilt 0 these curves are the icon's own
// lines. The detail is the rest of a 15 degree grid, the polar axis and a
// tilted orbit. Whatever is behind the sphere draws faint.
const R = 21;
const RAD = Math.PI / 180;
const meridian = (lon) => (u) => [
  R * Math.cos(u) * Math.sin(lon * RAD),
  -R * Math.sin(u),
  R * Math.cos(u) * Math.cos(lon * RAD),
];
const parallel = (lat) => (u) => [
  R * Math.cos(lat * RAD) * Math.sin(u),
  -R * Math.sin(lat * RAD),
  R * Math.cos(lat * RAD) * Math.cos(u),
];
const ICON = [...[0, 45, -45, 90].map(meridian), ...[0, 30, -30].map(parallel)];
const ORBIT = { r: 1.45 * R, tilt: 18 * RAD };
const DETAIL = [
  ...[15, 30, 60, 75, -15, -30, -60, -75].map(meridian),
  ...[15, 45, 60, 75, -15, -45, -60, -75].map(parallel),
  (u) => [0, R * 1.3 * (u / Math.PI - 1), 0],
  (u) => [
    ORBIT.r * Math.cos(u) * Math.cos(ORBIT.tilt),
    ORBIT.r * Math.cos(u) * Math.sin(ORBIT.tilt),
    ORBIT.r * Math.sin(u),
  ],
];

function sphere(curves, yaw, tilt) {
  const [cy, sy] = [Math.cos(yaw * RAD), Math.sin(yaw * RAD)];
  const [ct, st] = [Math.cos(tilt * RAD), Math.sin(tilt * RAD)];
  let front = "";
  let back = "";
  for (const curve of curves) {
    let last;
    for (let i = 0; i <= 96; i++) {
      const [x, y, z] = curve((i / 96) * 2 * Math.PI);
      const x1 = x * cy + z * sy;
      const z1 = z * cy - x * sy;
      const pt = `${(32 + x1).toFixed(2)} ${(32 + y * ct - z1 * st).toFixed(2)}`;
      const depth = y * st + z1 * ct;
      if (last) {
        if (last.depth + depth >= 0) front += `M${last.pt}L${pt}`;
        else back += `M${last.pt}L${pt}`;
      }
      last = { pt, depth };
    }
  }
  return [front, back];
}

// each key springs from where the previous key had got to when it started,
// so a move that interrupts another does not jump
function sample(keys, t, mix) {
  let pose = { ...REST, ...keys[0][1] };
  let seg;
  for (const [beat, target, springs] of keys.slice(1)) {
    const t0 = beat * BEAT;
    if (t < t0) break;
    seg = { from: seg ? mix(seg, t0) : pose, to: { ...REST, ...target }, t0, springs };
  }
  return seg ? mix(seg, t) : pose;
}

export function timeline(root, { reduce = false } = {}) {
  const scenes = [...root.querySelectorAll("[data-from]")];
  const cues = [...root.querySelectorAll("[data-at]")];
  const tracks = TRACKS.map(([sel, keys]) => ({ el: root.querySelector(sel), keys }));

  // content rises in on a critically damped spring, siblings 60 ms apart,
  // and every scene but the two signs leaves upward the same way, so the
  // whole film flows in one direction. A sign does not move, it switches.
  for (const scene of scenes) {
    const lit = scene.classList.contains("lit");
    const from = +scene.dataset.from;
    const entering = new Set(lit ? [] : [...scene.children].filter((el) => !(el.children.length && [...el.children].every((c) => c.dataset.at))));
    for (const el of scene.querySelectorAll("[data-at]")) entering.add(el);
    const count = new Map();
    for (const el of entering) {
      if (el.classList.contains("sign") || el instanceof SVGElement) continue;
      const beat = el.dataset.at ? +el.dataset.at : from;
      const i = count.get(beat) ?? 0;
      count.set(beat, i + 1);
      tracks.push({ el, keys: [[0, RISE], [beat + (i * STAGGER) / BEAT, {}, ENTER]] });
    }
    if (!lit) tracks.push({ el: scene, keys: [[0, {}], [+scene.dataset.to - 0.5, { y: -24, o: 0 }, EXIT]] });
  }

  // reduced motion in the preview: no movement, a short fade instead
  const mix = ({ from, to, t0, springs: [move, turn] }, t) =>
    Object.fromEntries(
      Object.keys(REST).map((k) => {
        const s = t - t0;
        const f = reduce ? (k === "o" ? Math.min(1, s / 0.2) : 1) : (TURNS.has(k) ? turn : move)(s);
        return [k, from[k] + (to[k] - from[k]) * f];
      }),
    );

  const lines = ["globe-front", "globe-back", "detail-front", "detail-back"].map((c) => root.querySelector(`.${c}`));
  const signs = [...root.querySelectorAll(".sign")];
  const tubes = [...root.querySelectorAll(".tube")];
  const typed = [...root.querySelectorAll("[data-type]")].map((el) => {
    const [from, to] = el.dataset.type.split(" ").map(Number);
    const head = el.querySelector(".hrp-part");
    const rest = el.querySelector(".rest");
    const full = head.textContent + rest.textContent;
    return { from, to, head, rest, full, split: head.textContent.length, cursor: el.querySelector(".live .cursor") };
  });

  return (t) => {
    const beat = t / BEAT;
    for (const el of scenes) el.hidden = !(beat >= +el.dataset.from && beat < +el.dataset.to);
    for (const el of cues) {
      const on = beat >= +el.dataset.at && !(beat >= +el.dataset.until);
      el.style.visibility = on ? "visible" : "hidden";
    }

    for (const { el, keys } of tracks) {
      const p = sample(keys, t, mix);
      el.style.transform = css(p);
      el.style.opacity = String(p.o);
    }

    // the strokes thin as the globe grows, so they keep one width on screen
    // and land at the icon's 2.25
    const g = globe(t, reduce);
    const width = 2.25 / g.scale;
    const paths = [...sphere(ICON, g.yaw, g.tilt), ...sphere(DETAIL, g.yaw, g.tilt)];
    const looks = [[1, width], [0.3, width], [0.55 * g.detail, width / 2], [0.15 * g.detail, width / 2]];
    lines.forEach((el, i) => {
      el.setAttribute("d", paths[i]);
      el.style.opacity = String(looks[i][0]);
      el.style.strokeWidth = String(looks[i][1]);
    });

    const lit = ignite(t);
    for (const el of signs) Object.assign(el.style, lit);
    const o = String(tube(t));
    for (const el of tubes) el.style.opacity = o;

    for (const s of typed) {
      const p = Math.min(1, Math.max(0, (beat - s.from) / (s.to - s.from)));
      const n = Math.round(p * s.full.length);
      s.head.textContent = s.full.slice(0, Math.min(n, s.split));
      s.rest.textContent = s.full.slice(s.split, Math.max(n, s.split));
      const typing = beat >= s.from && beat < s.to;
      s.cursor.style.visibility = typing || beat % 1 < 0.5 ? "visible" : "hidden";
    }
  };
}
