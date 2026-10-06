// renders index.html to out/zcash-disclosure.mp4: the soundtrack from an
// OfflineAudioContext in the page, then every frame at 60 fps as a 1920x1080
// screenshot piped into ffmpeg. With --preview it only serves the page.
import { existsSync } from "node:fs";
import { createServer } from "node:http";
import { once } from "node:events";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { extname, join, normalize } from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const FPS = 60;
const ROOT = fileURLToPath(new URL("..", import.meta.url));
const OUT = fileURLToPath(new URL("out/", import.meta.url));
// FFMPEG wins, then a binary dropped in motion/bin/, then Homebrew's, then PATH
const FFMPEG =
  process.env.FFMPEG ??
  [fileURLToPath(new URL("bin/ffmpeg", import.meta.url)), "/opt/homebrew/bin/ffmpeg"].find((path) => existsSync(path)) ??
  "ffmpeg";

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
  ".mp3": "audio/mpeg",
  ".json": "application/json",
};

// the repo root, so the page reaches the site's fonts and sound by relative path
function serve(port) {
  const server = createServer(async (req, res) => {
    const path = join(ROOT, normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)));
    try {
      if (!path.startsWith(ROOT)) throw new Error("outside the repo");
      const body = await readFile(path);
      res.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" });
      res.end(body);
    } catch {
      res.writeHead(404).end();
    }
  });
  server.listen(port, "127.0.0.1");
  return once(server, "listening").then(() => ({
    server,
    url: `http://127.0.0.1:${server.address().port}/motion/index.html`,
  }));
}

async function headlessShell() {
  if (process.env.CHROMIUM) return process.env.CHROMIUM;
  const cache = join(homedir(), "Library/Caches/ms-playwright");
  const builds = (await readdir(cache))
    .filter((d) => d.startsWith("chromium_headless_shell-"))
    .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  if (!builds.length) throw new Error(`no chromium headless shell in ${cache}`);
  return join(cache, builds[0], "chrome-headless-shell-mac-arm64/chrome-headless-shell");
}

if (process.argv.includes("--preview")) {
  const { url } = await serve(5180);
  console.log(url);
} else {
  const { server, url } = await serve(0);
  const browser = await chromium.launch({
    executablePath: await headlessShell(),
    chromiumSandbox: false,
    args: ["--font-render-hinting=none"],
  });
  const page = await browser.newPage({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1.5,
    colorScheme: "dark",
  });
  page.on("pageerror", (e) => {
    throw e;
  });
  await page.goto(`${url}?render`);
  await page.evaluate(() => window.ready);
  const length = await page.evaluate(async () => (await import("./timeline.js")).LENGTH);
  await mkdir(OUT, { recursive: true });

  const still = process.argv.indexOf("--still");
  if (still > 0) {
    for (const t of process.argv.slice(still + 1)) {
      await page.evaluate((s) => window.seek(s), Number(t));
      await page.screenshot({ path: join(OUT, `still-${t}.png`) });
    }
    await browser.close();
    server.close();
    process.exit();
  }

  const wav = join(OUT, "zcash-disclosure.wav");
  await writeFile(wav, Buffer.from(await page.evaluate(() => window.mixdown()), "base64"));

  const ff = spawn(
    FFMPEG,
    [
      "-y", "-loglevel", "error",
      "-f", "image2pipe", "-framerate", String(FPS), "-c:v", "png", "-i", "-",
      "-i", wav,
      "-map", "0:v", "-map", "1:a",
      "-c:v", "libx264", "-preset", "slow", "-crf", "14", "-pix_fmt", "yuv420p",
      "-c:a", "aac", "-b:a", "256k",
      "-movflags", "+faststart", "-shortest",
      join(OUT, "zcash-disclosure.mp4"),
    ],
    { stdio: ["pipe", "inherit", "inherit"] },
  );
  const done = once(ff, "exit");

  const frames = Math.round(length * FPS);
  for (let i = 0; i < frames; i++) {
    await page.evaluate((t) => window.seek(t), i / FPS);
    if (!ff.stdin.write(await page.screenshot({ type: "png" }))) await once(ff.stdin, "drain");
    if (i % FPS === 0) process.stdout.write(`\r${i / FPS}/${Math.ceil(length)} s`);
  }
  ff.stdin.end();
  const [code] = await done;
  process.stdout.write("\n");
  await browser.close();
  server.close();
  if (code !== 0) throw new Error(`ffmpeg exited with ${code}`);
  console.log(join(OUT, "zcash-disclosure.mp4"));
}
