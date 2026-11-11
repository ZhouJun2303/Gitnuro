// Seed the lab, start it on a port that does not collide with `pnpm start`, then run Playwright.

import { readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { run, startLab, startVite, stop, waitFor } from "./lab-serve.mjs";

const port = 1422;
const args = process.argv.slice(2).filter((arg) => arg !== "--");
const reuse = args.includes("--reuse");
const selection = args.filter((arg) => arg !== "--reuse");
if (!reuse) await run(["scripts/lab-seed.mjs"]);
const lab = startLab();
const vite = startVite(port);

try {
  await waitFor("http://127.0.0.1:1429/health", 240);
  await waitFor(`http://127.0.0.1:${port}/`, 90);
  await ensureChromium();
  await run(["node_modules/@playwright/test/cli.js", "test", ...selection], process.env);
} finally {
  stop(lab);
  stop(vite);
}

function browserRoot() {
  if (process.env.PLAYWRIGHT_BROWSERS_PATH) return process.env.PLAYWRIGHT_BROWSERS_PATH;
  if (process.platform === "darwin") return join(homedir(), "Library", "Caches", "ms-playwright");
  if (process.platform === "linux") return join(homedir(), ".cache", "ms-playwright");
  return join(process.env.LOCALAPPDATA || join(homedir(), "AppData", "Local"), "ms-playwright");
}

function chromiumReady() {
  try {
    return readdirSync(browserRoot()).some((name) => name.startsWith("chromium-") || name.startsWith("chromium_headless_shell-"));
  } catch {
    return false;
  }
}

async function ensureChromium() {
  const root = browserRoot();
  if (chromiumReady()) {
    console.log(`playwright chromium: ${root}`);
    return;
  }
  console.log(`playwright chromium is missing in ${root}; installing once`);
  try {
    await run(["node_modules/@playwright/test/cli.js", "install", "chromium"]);
  } catch (error) {
    console.error(`Could not install Chromium into ${root}.`);
    console.error("If the error is EPERM, set PLAYWRIGHT_BROWSERS_PATH to a writable folder, then run: pnpm exec playwright install chromium");
    throw error;
  }
}
