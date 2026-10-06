// two ways in. With ?render the page waits to be driven: render.mjs calls
// seek(t) for every frame and mixdown() once for the soundtrack. Without it
// the page is a preview that plays the score live, with ?t=12.5 to hold a
// single frame.
import { LENGTH, timeline } from "./timeline.js";
import { decodeBuzz, score } from "./sound.js";

const BUZZ_URL = "../site/public/sound/neon-buzz.mp3";
const RATE = 48000;

const stage = document.querySelector(".stage");
const play = document.querySelector(".play");
const params = new URLSearchParams(location.search);
// the live preview honours reduced motion: every move lands on its first beat
const seek = timeline(stage, {
  reduce: !params.has("render") && matchMedia("(prefers-reduced-motion: reduce)").matches,
});

function wav(buffer) {
  const channels = buffer.numberOfChannels;
  const frames = buffer.length;
  const bytes = new DataView(new ArrayBuffer(44 + frames * channels * 2));
  const text = (at, s) => [...s].forEach((c, i) => bytes.setUint8(at + i, c.charCodeAt(0)));
  text(0, "RIFF");
  bytes.setUint32(4, 36 + frames * channels * 2, true);
  text(8, "WAVEfmt ");
  bytes.setUint32(16, 16, true);
  bytes.setUint16(20, 1, true);
  bytes.setUint16(22, channels, true);
  bytes.setUint32(24, buffer.sampleRate, true);
  bytes.setUint32(28, buffer.sampleRate * channels * 2, true);
  bytes.setUint16(32, channels * 2, true);
  bytes.setUint16(34, 16, true);
  text(36, "data");
  bytes.setUint32(40, frames * channels * 2, true);
  const data = [...Array(channels)].map((_, c) => buffer.getChannelData(c));
  let at = 44;
  for (let i = 0; i < frames; i++) {
    for (const ch of data) {
      const s = Math.max(-1, Math.min(1, ch[i]));
      bytes.setInt16(at, s < 0 ? s * 0x8000 : s * 0x7fff, true);
      at += 2;
    }
  }
  return new Uint8Array(bytes.buffer);
}

function base64(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

if (params.has("render")) {
  document.documentElement.classList.add("render");
  window.seek = seek;
  window.mixdown = async () => {
    const ctx = new OfflineAudioContext(2, Math.round(LENGTH * RATE), RATE);
    score(ctx, 0, await decodeBuzz(ctx, BUZZ_URL));
    return base64(wav(await ctx.startRendering()));
  };
  window.ready = document.fonts.ready.then(() => seek(0));
} else {
  const fit = () => {
    stage.style.zoom = String(Math.min(innerWidth / 1280, innerHeight / 720));
  };
  fit();
  addEventListener("resize", fit);
  seek(Number(params.get("t") ?? 0));

  play.hidden = false;
  play.addEventListener("click", async () => {
    play.hidden = true;
    const ctx = new AudioContext({ sampleRate: RATE });
    const buzz = await decodeBuzz(ctx, BUZZ_URL);
    const t0 = ctx.currentTime + 0.2;
    score(ctx, t0, buzz);
    const frame = () => {
      const t = ctx.currentTime - t0;
      seek(Math.max(0, t));
      if (t < LENGTH) return requestAnimationFrame(frame);
      void ctx.close();
      play.hidden = false;
    };
    requestAnimationFrame(frame);
  });
}
