import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { rm } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import test from "node:test";

let httpError = "";

test("desktop takes over a stopped borrowed server and restarts a failed Vite", { timeout: 60_000 }, async (t) => {
  // Exercise process ownership and recovery without compiling the app twice.
  mkdirSync(resolve(".lab"), { recursive: true });
  const fixture = mkdtempSync(resolve(".lab", "dev-server-test-"));
  mkdirSync(join(fixture, "scripts"));
  mkdirSync(join(fixture, "node_modules", "vite", "bin"), { recursive: true });
  copyFileSync("scripts/dev-server.mjs", join(fixture, "scripts", "dev-server.mjs"));
  writeFileSync(join(fixture, "node_modules", "vite", "bin", "vite.js"), `
    require("node:http").createServer((_, response) => response.end(String(process.pid)))
      .listen(Number(process.env.AWEGIT_VITE_PORT), "127.0.0.1");
  `);
  const borrowed = createServer((socket) => socket.destroy());
  borrowed.listen(0, "127.0.0.1");
  await once(borrowed, "listening");
  const port = borrowed.address().port;
  t.after(() => { if (borrowed.listening) borrowed.close(); });

  const monitor = spawn(process.execPath, [join(fixture, "scripts", "dev-server.mjs")], {
    env: { ...process.env, AWEGIT_VITE_PORT: String(port), TAURI_DEV_HOST: "", NO_COLOR: "1" },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let output = "";
  t.after(async () => {
    const closed = once(monitor, "close");
    stop(monitor.pid);
    for (const match of output.matchAll(/启动 Vite pid=(\d+)/g)) stop(Number(match[1]));
    await closed;
    await rm(fixture, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  });
  for (const stream of [monitor.stdout, monitor.stderr]) stream.on("data", (data) => { output += data; });

  await until(() => output.includes("沿用并监测"), () => output);
  await delay(1200);
  assert.equal(monitor.exitCode, null, "the desktop must keep monitoring a borrowed server");
  assert.doesNotMatch(output, /启动 Vite/, "the borrowed server must not be replaced while it is running");

  await new Promise((done) => borrowed.close(done));
  await until(() => output.includes("自动接管"), () => output);
  const url = `http://127.0.0.1:${port}/?view=flow`;
  await until(() => responds(url), () => output);
  const pid = Number(output.match(/启动 Vite pid=(\d+)/)?.[1]);
  assert.ok(pid > 0, output);
  stop(pid);

  await until(() => [...output.matchAll(/启动 Vite pid=(\d+)/g)].length >= 2, () => output);
  await until(() => responds(url), () => output);
  assert.match(output, /Vite 已退出 code=/, "record the exit before recovering");
  assert.equal(monitor.exitCode, null);
});

async function responds(url) {
  try {
    const response = await fetch(url, { signal: AbortSignal.timeout(3000) });
    httpError = `${url}: HTTP ${response.status}`;
    return response.ok;
  } catch (error) { httpError = `${url}: ${error.message} ${error.cause?.code ?? ""}`; return false; }
}

async function until(check, detail) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    if (await check()) return;
    await delay(100);
  }
  assert.fail(`development server did not recover: ${httpError}\n${detail()}`);
}

function stop(pid) {
  try { process.kill(pid, "SIGTERM"); } catch (error) { if (error.code !== "ESRCH") throw error; }
}
