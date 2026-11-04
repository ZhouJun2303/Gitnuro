// Seed the lab, start it on a port that does not collide with `pnpm start`, then run Playwright.

import { run, startLab, startVite, stop, waitFor } from "./lab-serve.mjs";

const port = 1422;
await run(["scripts/lab-seed.mjs"]);
const lab = startLab();
const vite = startVite(port);

try {
  await waitFor("http://127.0.0.1:1429/health", 240);
  await waitFor(`http://127.0.0.1:${port}/`, 90);
  await run(["node_modules/@playwright/test/cli.js", "install", "chromium"]);
  await run(["node_modules/@playwright/test/cli.js", "test"], process.env);
} finally {
  stop(lab);
  stop(vite);
}
