// a Berlin-school sequence over the drone, after Tangerine Dream and Tron:
// Legacy, at 124 BPM in A minor, one chord a bar through Am(add9), Am, Fmaj7
// and E7(b9). A sawtooth bass pulses root and octave in sixteenths through a
// resonant filter that opens over sixteen bars and closes over the next
// sixteen. After three bars a square-wave line joins above it. Its pattern is
// seven steps long, so against the sixteen steps of a bar it never lands the
// same way twice, and a dotted-eighth echo throws it to the other side.
// motion/sound.js plays the same sequence in the video.
const BPM = 124;
const STEP = 60 / BPM / 4;
const LOOKAHEAD = 0.12;

type Tones = [number, number, number, number, number];
const ROOTS: [number, ...number[]] = [33, 33, 29, 28];
const TONES: [Tones, ...Tones[]] = [
  [57, 60, 64, 71, 72],
  [57, 60, 64, 69, 72],
  [53, 57, 60, 64, 69],
  [52, 56, 59, 62, 65],
];
const BASS = [0, 12, 0, 0, 12, 0, 7, 12, 0, 12, 0, 0, 12, 0, 7, 12];
const LINE = [0, 2, 1, 3, 2, 4, 1];
const LINE_FROM = 3 * 16;
const SWEEP = [260, 1600] as const;
const SWEEP_STEPS = 16 * 16;

const hz = (midi: number) => 440 * 2 ** ((midi - 69) / 12);

function note(
  ctx: AudioContext,
  out: AudioNode,
  type: OscillatorType,
  midi: number,
  t: number,
  cutoff: number,
  q: number,
  level: number,
  length: number,
) {
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

function pan(ctx: AudioContext, value: number, out: AudioNode) {
  const node = ctx.createStereoPanner();
  node.pan.value = value;
  node.connect(out);
  return node;
}

// up over sixteen bars, back down over the next sixteen
function cutoff(step: number) {
  const phase = (step % (2 * SWEEP_STEPS)) / SWEEP_STEPS;
  const open = phase <= 1 ? phase : 2 - phase;
  return SWEEP[0] * (SWEEP[1] / SWEEP[0]) ** open;
}

export function sequencer(ctx: AudioContext, out: AudioNode) {
  const bass = ctx.createGain();
  bass.connect(out);
  const line = ctx.createGain();
  line.connect(pan(ctx, 0.3, out));

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

  let step = 0;
  let next = 0;
  let timer: number | undefined;
  const tick = () => {
    while (next < ctx.currentTime + LOOKAHEAD) {
      const bar = Math.floor(step / 16) % ROOTS.length;
      const root = ROOTS[bar] ?? ROOTS[0];
      const tones = TONES[bar] ?? TONES[0];
      const c = cutoff(step);
      const accent = step % 4 === 0 ? 1.4 : 1;
      note(ctx, bass, "sawtooth", root + (BASS[step % 16] ?? 0), next, c * accent, 9, 0.2, 0.13);
      if (step >= LINE_FROM) {
        note(ctx, line, "square", tones[LINE[step % LINE.length] ?? 0] ?? tones[0], next, c * 1.6, 4, 0.08, 0.2);
      }
      next += STEP;
      step += 1;
    }
  };

  return {
    start(at = ctx.currentTime + 0.05) {
      if (timer !== undefined) return;
      step = 0;
      next = at;
      timer = window.setInterval(tick, 25);
    },
    stop() {
      window.clearInterval(timer);
      timer = undefined;
    },
  };
}
