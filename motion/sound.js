// the site's sound engine (site/src/lib/buzz.ts and seq.ts) laid out on the
// score instead of on toggles: every node starts, and every gain change is
// scheduled, against t0 on the context's clock. The same graph plays live in
// an AudioContext and renders in an OfflineAudioContext.
import { BEAT, BUZZ, FAULTS, IGNITE, OFF, SEQ } from "./timeline.js";

const LEVEL = 0.35;

const A0 = 27.5;
const A1 = 55;
const C2 = 65.41;
const E2 = 82.41;
const F2 = 87.31;

const VOICES = [
  ["sine", A0, 1],
  ["sawtooth", A1, 0.3],
  ["sawtooth", A1 + 0.25, 0.3],
  ["sawtooth", C2, 0.22],
  ["triangle", E2, 0.2],
];

const STEP = BEAT / 4;

// a Berlin-school sequence, after Tangerine Dream and Tron: Legacy, over the
// same dark harmony as before, Am(add9), Am, Fmaj7, E7(b9), one chord a bar.
// A sawtooth bass pulses root and octave in sixteenths through a resonant
// filter that opens over the whole phrase. From bar 7 a square-wave line
// joins above it. Its pattern is seven steps long, so against the sixteen
// steps of a bar it never lands the same way twice. site/src/lib/seq.ts
// plays the same sequence on the site.
const ROOTS = [33, 33, 29, 28];
const TONES = [
  [57, 60, 64, 71, 72],
  [57, 60, 64, 69, 72],
  [53, 57, 60, 64, 69],
  [52, 56, 59, 62, 65],
];
const BASS = [0, 12, 0, 0, 12, 0, 7, 12, 0, 12, 0, 0, 12, 0, 7, 12];
const LINE = [0, 2, 1, 3, 2, 4, 1];
const LINE_FROM = 28;
const SWEEP = [260, 1600];

const hz = (midi) => 440 * 2 ** ((midi - 69) / 12);

function lfo(ctx, t0, rate, depth, target) {
  const osc = ctx.createOscillator();
  osc.frequency.value = rate;
  const amount = ctx.createGain();
  amount.gain.value = depth;
  osc.connect(amount).connect(target);
  osc.start(t0);
}

function voice(ctx, t0, type, freq, amp, out) {
  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.value = freq;
  const level = ctx.createGain();
  level.gain.value = amp;
  osc.connect(level).connect(out);
  osc.start(t0);
  return { osc, level };
}

function drone(ctx, t0, out) {
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 150;
  filter.Q.value = 3;
  const level = ctx.createGain();
  level.gain.value = 0.6;
  filter.connect(level).connect(out);
  lfo(ctx, t0, 0.05, 40, filter.frequency);

  for (const [type, freq, amp] of VOICES) {
    const { osc } = voice(ctx, t0, type, freq, amp, filter);
    if (type === "sawtooth") lfo(ctx, t0, 0.2, 0.35, osc.frequency);
  }

  const sixth = voice(ctx, t0, "sawtooth", F2, 0.12, filter);
  lfo(ctx, t0, 1 / 40, 0.12, sixth.level.gain);
}

function note(ctx, out, type, midi, t, cutoff, q, level, length) {
  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.value = hz(midi);
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.Q.value = q;
  filter.frequency.setValueAtTime(cutoff * 4, t);
  filter.frequency.exponentialRampToValueAtTime(cutoff, t + length * 0.6);
  const amp = ctx.createGain();
  amp.gain.setValueAtTime(0, t);
  amp.gain.linearRampToValueAtTime(level, t + 0.004);
  amp.gain.exponentialRampToValueAtTime(0.001, t + length);
  osc.connect(filter).connect(amp).connect(out);
  osc.start(t);
  osc.stop(t + length + 0.02);
}

const pan = (ctx, value, out) => {
  const node = ctx.createStereoPanner();
  node.pan.value = value;
  node.connect(out);
  return node;
};

function sequencer(ctx, t0, out) {
  const bass = ctx.createGain();
  bass.connect(out);
  const line = ctx.createGain();
  line.connect(pan(ctx, 0.3, out));

  // a dotted-eighth echo of the line, thrown to the other side
  const delay = ctx.createDelay(1);
  delay.delayTime.value = STEP * 3;
  const tone = ctx.createBiquadFilter();
  tone.type = "lowpass";
  tone.frequency.value = 1800;
  const feedback = ctx.createGain();
  feedback.gain.value = 0.42;
  const wet = ctx.createGain();
  wet.gain.value = 0.45;
  delay.connect(tone).connect(feedback).connect(delay);
  delay.connect(wet).connect(pan(ctx, -0.45, out));
  line.connect(delay);

  const [from, to] = SEQ;
  const steps = (to - from) * 4;
  for (let step = 0; step < steps; step++) {
    const t = t0 + from * BEAT + step * STEP;
    const bar = Math.floor(step / 16) % ROOTS.length;
    const cutoff = SWEEP[0] * (SWEEP[1] / SWEEP[0]) ** (step / steps);
    const accent = step % 4 === 0 ? 1.4 : 1;
    note(ctx, bass, "sawtooth", ROOTS[bar] + BASS[step % 16], t, cutoff * accent, 9, 0.2, 0.13);
    if (from + step / 4 >= LINE_FROM) {
      note(ctx, line, "square", TONES[bar][LINE[step % LINE.length]], t, cutoff * 1.6, 4, 0.08, 0.2);
    }
  }
}

function buzz(ctx, t0, buffer, out) {
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.loop = true;
  source.connect(out);
  source.start(t0);
}

const CLICK = 0.006;

export async function decodeBuzz(ctx, url) {
  const bytes = await fetch(url).then((r) => r.arrayBuffer());
  return ctx.decodeAudioData(bytes);
}

export function score(ctx, t0, buzzBuffer) {
  const master = ctx.createGain();
  master.gain.value = LEVEL;
  const limiter = ctx.createDynamicsCompressor();
  limiter.threshold.value = -12;
  limiter.knee.value = 6;
  limiter.ratio.value = 12;
  limiter.attack.value = 0.003;
  limiter.release.value = 0.25;
  master.connect(limiter).connect(ctx.destination);

  const channel = () => {
    const node = ctx.createGain();
    node.gain.value = 0;
    node.connect(master);
    return node;
  };

  const on = IGNITE * BEAT + t0;

  const droneCh = channel();
  drone(ctx, t0, droneCh);
  droneCh.gain.setTargetAtTime(1, on, 0.4);

  const seqCh = channel();
  seqCh.gain.value = 1;
  sequencer(ctx, t0, seqCh);

  const buzzCh = channel();
  buzz(ctx, t0, buzzBuffer, buzzCh);
  for (const [from, to] of BUZZ) {
    buzzCh.gain.setValueAtTime(0, t0 + from * BEAT);
    buzzCh.gain.linearRampToValueAtTime(1, t0 + from * BEAT + CLICK);
    buzzCh.gain.setValueAtTime(1, t0 + to * BEAT);
    buzzCh.gain.linearRampToValueAtTime(0, t0 + to * BEAT + CLICK);
  }

  // a fault makes the tube arc: the sound jumps, then settles with the light
  for (const f of FAULTS) {
    const at = t0 + f.at;
    master.gain.setValueAtTime(LEVEL, at);
    master.gain.linearRampToValueAtTime(LEVEL * 2.4, at + 0.03);
    master.gain.setTargetAtTime(LEVEL, at + f.dur, 0.08);
  }

  const off = t0 + OFF * BEAT;
  master.gain.setValueAtTime(LEVEL, off);
  master.gain.linearRampToValueAtTime(0, off + CLICK);
}
