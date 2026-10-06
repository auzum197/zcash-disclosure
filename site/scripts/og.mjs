// draws the link preview images from scripts/og.html: one 1200x630 PNG per
// route into public/og/, and the 180x180 apple-touch-icon.png. The page runs
// on Vite's dev server, so it reads the routes from src/meta.ts and the
// fonts from public/.
import { mkdir, readdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const PAGES = ["home", "create", "inspect", "verify"];

async function headlessShell() {
  if (process.env.CHROMIUM) return process.env.CHROMIUM;
  const cache = join(homedir(), "Library/Caches/ms-playwright");
  const builds = (await readdir(cache))
    .filter((d) => d.startsWith("chromium_headless_shell-"))
    .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  if (!builds.length) throw new Error(`no chromium headless shell in ${cache}`);
  return join(cache, builds[0], "chrome-headless-shell-mac-arm64/chrome-headless-shell");
}

const server = await createServer({ root: ROOT, server: { port: 0, host: "127.0.0.1" }, logLevel: "error" });
await server.listen();
const base = server.resolvedUrls.local[0];

const browser = await chromium.launch({ executablePath: await headlessShell(), chromiumSandbox: false });
await mkdir(join(ROOT, "public/og"), { recursive: true });

const shoot = async (page, size, path) => {
  const tab = await browser.newPage({ viewport: size, deviceScaleFactor: 1 });
  await tab.goto(`${base}scripts/og.html?page=${page}`);
  await tab.waitForFunction(() => window.ready);
  await tab.screenshot({ path, clip: { x: 0, y: 0, ...size } });
  await tab.close();
  console.log(path.slice(ROOT.length));
};

for (const page of PAGES) {
  await shoot(page, { width: 1200, height: 630 }, join(ROOT, `public/og/${page}.png`));
}
await shoot("touch", { width: 180, height: 180 }, join(ROOT, "public/apple-touch-icon.png"));

await browser.close();
await server.close();
