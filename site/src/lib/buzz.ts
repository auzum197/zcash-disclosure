// the sound of the sign: a looped buzz recording, a synth drone and the
// sequence, each on its own channel under one master gain. The browser only
// lets audio start from a user gesture, so nothing here runs until the sound
// button is pressed.
import { sequencer } from "./seq";

const LEVEL = 0.35;

// the order the sound comes in, on the audio clock: the tube strikes, two bars
// later the drone swells under it and the buzz falls far back, and two bars
// after that the sequence starts quietly and climbs for eight bars to its full
// level, where it stays
const BAR = (60 / 124) * 4;
const DRONE_AT = 2 * BAR;
const SEQ_AT = 4 * BAR;
const SEQ_RISE = 8 * BAR;
const BUZZ_UNDER = 0.1;
const SEQ_FLOOR = 0.03;

type Channel = "buzz" | "drone" | "seq";

let ctx: AudioContext | undefined;
let master: GainNode | undefined;
let seq: ReturnType<typeof sequencer> | undefined;
let channels: Record<Channel, GainNode> | undefined;
let playing = false;

// the drone under the buzz, in A minor voiced low: a sub sine on A0, the root
// as two detuned saws, the minor third as a saw, the fifth as a triangle. A
// minor sixth swells in and out against the fifth on a forty second cycle,
// a semitone apart, which is where the tension comes from. The saws wobble
// a little in pitch, and a low-pass closes the whole thing down under 160 Hz.
const A0 = 27.5;
const A1 = 55;
const C2 = 65.41;
const E2 = 82.41;
const F2 = 87.31;

const VOICES: Array<[OscillatorType, number, number]> = [
  ["sine", A0, 1],
  ["sawtooth", A1, 0.3],
  ["sawtooth", A1 + 0.25, 0.3],
  ["sawtooth", C2, 0.22],
  ["triangle", E2, 0.2],
];

function lfo(context: AudioContext, hz: number, depth: number, target: AudioParam) {
  const osc = context.createOscillator();
  osc.frequency.value = hz;
  const amount = context.createGain();
  amount.gain.value = depth;
  osc.connect(amount).connect(target);
  osc.start();
}

function voice(context: AudioContext, type: OscillatorType, hz: number, amp: number, out: AudioNode) {
  const osc = context.createOscillator();
  osc.type = type;
  osc.frequency.value = hz;
  const level = context.createGain();
  level.gain.value = amp;
  osc.connect(level).connect(out);
  osc.start();
  return { osc, level };
}

function drone(context: AudioContext, out: AudioNode) {
  const filter = context.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 150;
  filter.Q.value = 3;
  const level = context.createGain();
  level.gain.value = 0.6;
  filter.connect(level).connect(out);
  lfo(context, 0.05, 40, filter.frequency);

  for (const [type, hz, amp] of VOICES) {
    const { osc } = voice(context, type, hz, amp, filter);
    if (type === "sawtooth") lfo(context, 0.2, 0.35, osc.frequency);
  }

  const sixth = voice(context, "sawtooth", F2, 0.12, filter);
  lfo(context, 1 / 40, 0.12, sixth.level.gain);
}

async function buzz(context: AudioContext, out: AudioNode) {
  const bytes = await fetch("/sound/neon-buzz.mp3").then((r) => r.arrayBuffer());
  const source = context.createBufferSource();
  source.buffer = await context.decodeAudioData(bytes);
  source.loop = true;
  source.connect(out);
  source.start();
}

function graph() {
  ctx ??= new AudioContext();
  if (master) return { context: ctx, master };
  master = ctx.createGain();
  master.gain.value = LEVEL;
  const limiter = ctx.createDynamicsCompressor();
  limiter.threshold.value = -12;
  limiter.knee.value = 6;
  limiter.ratio.value = 12;
  limiter.attack.value = 0.003;
  limiter.release.value = 0.25;
  master.connect(limiter).connect(ctx.destination);
  return { context: ctx, master };
}

async function build() {
  const { context, master } = graph();
  if (channels) return channels;
  const node = () => {
    const gain = context.createGain();
    gain.gain.value = 0;
    gain.connect(master);
    return gain;
  };
  const built = { buzz: node(), drone: node(), seq: node() };
  drone(context, built.drone);
  seq = sequencer(context, built.seq);
  await buzz(context, built.buzz);
  channels = built;
  return built;
}

export async function play() {
  playing = true;
  const nodes = await build();
  if (!playing || !ctx) return;
  await ctx.resume();
  const t = ctx.currentTime + 0.05;
  for (const { gain } of Object.values(nodes)) {
    gain.cancelScheduledValues(t);
    gain.setValueAtTime(gain.value, t);
  }
  nodes.buzz.gain.setTargetAtTime(1, t, 0.05);
  nodes.buzz.gain.setTargetAtTime(BUZZ_UNDER, t + DRONE_AT, 0.5);
  nodes.drone.gain.setTargetAtTime(1, t + DRONE_AT, 0.4);
  nodes.seq.gain.setValueAtTime(0, t + SEQ_AT - 0.01);
  nodes.seq.gain.setValueAtTime(SEQ_FLOOR, t + SEQ_AT);
  nodes.seq.gain.exponentialRampToValueAtTime(1, t + SEQ_AT + SEQ_RISE);
  seq?.start(t + SEQ_AT);
}

export function stop() {
  playing = false;
  if (!ctx || !channels) return;
  const t = ctx.currentTime;
  for (const { gain } of Object.values(channels)) {
    gain.cancelScheduledValues(t);
    gain.setValueAtTime(gain.value, t);
    gain.setTargetAtTime(0, t, 0.12);
  }
  seq?.stop();
  window.setTimeout(() => {
    if (!playing) void ctx?.suspend();
  }, 600);
}

// a fault makes the tube arc: the sound jumps, then settles with the light
export function buzzFault(ms: number) {
  if (!playing || !ctx || !master) return;
  const t = ctx.currentTime;
  master.gain.cancelScheduledValues(t);
  master.gain.setValueAtTime(master.gain.value, t);
  master.gain.linearRampToValueAtTime(LEVEL * 2.4, t + 0.03);
  master.gain.setTargetAtTime(LEVEL, t + ms / 1000, 0.08);
}
