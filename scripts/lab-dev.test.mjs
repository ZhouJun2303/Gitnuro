import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { rm } from "node:fs/promises";
import { createServer } from "node:http";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import test from "node:test";

test("lab reuses an occupied frontend port and leaves the borrowed server running on exit", { timeout: 30_000 }, async (t) => {
  const server = await borrowedServer(t);
  const lab = launch(t, server.address().port);
  await until(() => lab.output.includes("沿用并监测") && lab.output.includes("fixture lab pid="), lab);
  assert.equal(await page(server.address().port), "borrowed");
  assert.equal(lab.child.exitCode, null);
  assert.doesNotMatch(lab.output, /启动 Vite|Port .*already in use/);

  stop(lab.backendPid());
  await lab.closed;
  assert.equal(await page(server.address().port), "borrowed");
});

test("lab takes over a stopped borrowed frontend and cleans up its own Vite on exit", { timeout: 30_000 }, async (t) => {
  const server = await borrowedServer(t);
  const port = server.address().port;
  const lab = launch(t, port);
  await until(() => lab.output.includes("沿用并监测") && lab.output.includes("fixture lab pid="), lab);

  await new Promise((done) => server.close(done));
  await until(() => lab.output.includes("自动接管") && lab.output.includes("fixture Vite pid="), lab);
  const vitePid = Number(lab.output.match(/fixture Vite pid=(\d+)/)?.[1]);
  assert.equal(await page(port), String(vitePid));
  assert.equal(lab.child.exitCode, null);

  stop(lab.backendPid());
  await lab.closed;
  await until(() => !alive(vitePid), lab);
});

async function borrowedServer(t) {
  const server = createServer((_, response) => response.end("borrowed"));
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => new Promise((done) => server.listening ? server.close(done) : done()));
  return server;
}

function launch(t, port) {
  // Exercise the actual pnpm dev entry point without compiling or touching live lab repositories.
  mkdirSync(resolve(".lab"), { recursive: true });
  const fixture = mkdtempSync(resolve(".lab", "lab-dev-test-"));
  mkdirSync(join(fixture, "scripts"));
  mkdirSync(join(fixture, ".lab", "repos", "full", ".git"), { recursive: true });
  mkdirSync(join(fixture, "node_modules", "vite", "bin"), { recursive: true });
  for (const file of ["lab-dev.mjs", "lab-serve.mjs", "dev-server.mjs"]) {
    copyFileSync(join("scripts", file), join(fixture, "scripts", file));
  }
  writeFileSync(join(fixture, "scripts", "with-rust-path.mjs"), `
    import { createServer } from "node:http";
    createServer((_, response) => response.end("lab")).listen(0, "127.0.0.1", () => {
      console.log("fixture lab pid=" + process.pid);
    });
  `);
  writeFileSync(join(fixture, "node_modules", "vite", "bin", "vite.js"), `
    require("node:http").createServer((_, response) => response.end(String(process.pid)))
      .listen(Number(process.env.AWEGIT_VITE_PORT), "127.0.0.1", () => {
        console.log("fixture Vite pid=" + process.pid);
      });
  `);
  const child = spawn(process.execPath, [join(fixture, "scripts", "lab-dev.mjs")], {
    env: { ...process.env, AWEGIT_VITE_PORT: String(port), TAURI_DEV_HOST: "", NO_COLOR: "1" },
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });
  const lab = { child, output: "", closed: once(child, "close") };
  lab.backendPid = () => Number(lab.output.match(/fixture lab pid=(\d+)/)?.[1]);
  for (const stream of [child.stdout, child.stderr]) stream.on("data", (data) => { lab.output += data; });
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      if (process.platform === "win32") {
        const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore", windowsHide: true });
        await once(killer, "close");
      } else {
        child.kill("SIGTERM");
      }
    }
    for (const match of lab.output.matchAll(/(?:fixture lab|fixture Vite|启动 Vite) pid=(\d+)/g)) {
      stop(Number(match[1]));
    }
    await lab.closed;
    await rm(fixture, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  });
  return lab;
}

async function page(port) {
  const response = await fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(3000) });
  assert.equal(response.status, 200);
  return response.text();
}

async function until(check, lab) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (await check()) return;
    await delay(50);
  }
  assert.fail(`lab did not reach the expected state:\n${lab.output}`);
}

function alive(pid) {
  try { process.kill(pid, 0); return true; } catch (error) { if (error.code !== "ESRCH") throw error; return false; }
}

function stop(pid) {
  if (!pid) return;
  try { process.kill(pid, "SIGTERM"); } catch (error) { if (error.code !== "ESRCH") throw error; }
}
