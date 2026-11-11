// Keep the desktop's frontend available even when a reused lab server stops.

import { spawn } from "node:child_process";
import { createWriteStream, mkdirSync } from "node:fs";
import { createConnection } from "node:net";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const host = process.env.TAURI_DEV_HOST || "127.0.0.1";
const port = Number(process.env.AWEGIT_VITE_PORT || 1420);
const logDir = join(root, ".lab", "logs");
mkdirSync(logDir, { recursive: true });
const output = createWriteStream(join(logDir, "dev-server.log"), { flags: "a" });
output.on("error", (error) => console.error(`开发服务器日志写入失败：${error.message}`));
const abort = new AbortController();
let child;
let stopping = false;

function log(message) {
  const line = `${new Date().toISOString()} [dev-server] ${message}\n`;
  process.stdout.write(line);
  if (!output.destroyed) output.write(line);
}

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    stopping = true;
    abort.abort();
    child?.kill(signal);
  });
}

try {
  let reused = false;
  while (!stopping) {
    if (await listening(host, port)) {
      if (!reused) log(`http://${host}:${port}/ 已在运行，沿用并监测这个开发服务器。`);
      reused = true;
      await pause();
      continue;
    }
    if (stopping) break;
    if (reused) log("沿用的开发服务器已停止，自动接管。");
    reused = false;
    child = spawn(process.execPath, [join(root, "node_modules", "vite", "bin", "vite.js"), "dev"], {
      cwd: root,
      // Vite treats stdin EOF as shutdown; keep this pipe open until we stop.
      stdio: ["pipe", "pipe", "pipe"],
      env: process.env,
      windowsHide: true,
    });
    log(`启动 Vite pid=${child.pid ?? "unknown"} port=${port}`);
    for (const [source, target] of [[child.stdout, process.stdout], [child.stderr, process.stderr]]) {
      source.pipe(target, { end: false });
      source.on("data", (data) => { if (!output.destroyed) output.write(data); });
    }
    await new Promise((done) => {
      child.on("error", (error) => log(`Vite 启动失败：${error.message}`));
      child.on("close", (code, signal) => {
        log(`Vite 已退出 code=${code} signal=${signal ?? "none"}`);
        done();
      });
    });
    child = undefined;
    if (!stopping) await pause();
  }
} finally {
  log("停止监测开发服务器。");
  output.end();
}

async function pause() {
  await delay(1000, undefined, { signal: abort.signal }).catch((error) => {
    if (error.name !== "AbortError") throw error;
  });
}

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
