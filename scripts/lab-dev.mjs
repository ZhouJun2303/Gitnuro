// Browser lab: seed on first use, then the command server and the Vite home page.

import { existsSync } from "node:fs";
import { join } from "node:path";
import { root, run, startLab, startVite, stop } from "./lab-serve.mjs";

if (!existsSync(join(root, ".lab", "repos", "full", ".git"))) {
  await run(["scripts/lab-seed.mjs"]);
}

const lab = startLab();
const vite = startVite(process.env.AWEGIT_VITE_PORT || "1420", { reuse: true });

function shutdown() {
  stop(lab);
  stop(vite);
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    shutdown();
    process.exit(0);
  });
}

lab.on("exit", (code) => {
  stop(vite);
  process.exit(code ?? 1);
});
vite.on("exit", (code) => {
  stop(lab);
  process.exit(code ?? 1);
});
