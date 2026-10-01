// Start Vite for `tauri dev`. If 1420 is already open, leave that server running.
// A second `pnpm start` used to fail because Vite binds this port with strictPort.

import { spawn } from "node:child_process";
import { createConnection } from "node:net";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const host = process.env.TAURI_DEV_HOST || "127.0.0.1";
const port = 1420;

if (await listening(host, port)) {
  console.log(`http://${host}:${port}/ 已在运行，沿用这个开发服务器。`);
  process.exit(0);
}

const child = spawn(process.execPath, [join(root, "node_modules", "vite", "bin", "vite.js"), "dev"], {
  cwd: root,
  stdio: "inherit",
  env: process.env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});

function listening(host, port) {
  return new Promise((resolve) => {
    const socket = createConnection({ host, port });
    const done = (open) => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(open);
    };
    socket.setTimeout(400);
    socket.once("connect", () => done(true));
    socket.once("timeout", () => done(false));
    socket.once("error", () => done(false));
  });
}
