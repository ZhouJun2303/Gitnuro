import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import test from "node:test";
import { createServer, resolveConfig } from "vite";
import { isolatedViteCache } from "./vite-cache.mjs";

async function fixture(t) {
  await mkdir(resolve(".lab"), { recursive: true });
  const root = await mkdtemp(resolve(".lab", "vite-cache-test-"));
  t.after(() => rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }));
  await writeFile(join(root, "package.json"), '{"type":"module"}');
  await writeFile(join(root, "index.html"), '<script type="module" src="/entry.js"></script>');
  await writeFile(join(root, "entry.js"), 'import { get, writable } from "svelte/store"; export const value = get(writable(42));');
  return root;
}

function config(root, extra = {}) {
  return { root, configFile: false, logLevel: "silent", plugins: [isolatedViteCache()], ...extra };
}

test("Vite keeps reusable desktop, UI and build caches separate", async (t) => {
  const root = await fixture(t);
  const desktop = await resolveConfig(config(root, { server: { port: 1420 } }), "serve");
  const ui = await resolveConfig(config(root, { server: { port: 1422 } }), "serve");
  const build = await resolveConfig(config(root), "build");
  const restarted = await resolveConfig(config(root, { server: { port: 1420 } }), "serve");
  assert.equal(new Set([desktop.cacheDir, ui.cacheDir, build.cacheDir]).size, 3);
  assert.equal(restarted.cacheDir, desktop.cacheDir, "restarting one port should reuse its warm cache");
});

test("Vite gives temporary and middleware servers separate caches and respects explicit caches", async (t) => {
  const root = await fixture(t);
  const temporary = await resolveConfig(config(root, { server: { port: 0 } }), "serve");
  const another = await resolveConfig(config(root, { server: { port: 0 } }), "serve");
  const middleware = await resolveConfig(config(root, { server: { middlewareMode: true } }), "serve");
  assert.equal(new Set([temporary.cacheDir, another.cacheDir, middleware.cacheDir]).size, 3);
  const explicit = join(root, "explicit-cache");
  const overridden = await resolveConfig(config(root, { cacheDir: explicit }), "serve");
  assert.equal(overridden.cacheDir, explicit.replaceAll("\\", "/"));
});

test("a second Vite prebundle cannot invalidate the first server's SSR store module", { timeout: 60_000 }, async (t) => {
  const servers = [];
  t.after(async () => {
    for (const server of servers.reverse()) await server.close();
  });
  const root = await fixture(t);
  async function start(dependency) {
    const server = await createServer(config(root, {
      server: { host: "127.0.0.1", port: 0, hmr: false, watch: null },
      optimizeDeps: { noDiscovery: true },
      ssr: { noExternal: ["svelte"], optimizeDeps: { include: [dependency], noDiscovery: true, force: true } },
    }));
    servers.push(server);
    await server.listen();
    return server;
  }

  const first = await start("svelte/store");
  assert.equal((await first.ssrLoadModule("/entry.js")).value, 42);
  const optimizer = first.environments.ssr.depsOptimizer;
  const store = optimizer.metadata.optimized["svelte/store"];
  const id = optimizer.getOptimizedDepId(store);
  const before = await readFile(store.file, "utf8");

  // A different include list makes a shared cache remove the first server's store bundle.
  const second = await start("svelte/transition");
  await second.ssrLoadModule("svelte/transition");
  const loaded = await first.environments.ssr.pluginContainer.load(id);
  assert.equal(loaded.code, before);
  assert.equal(await readFile(store.file, "utf8"), before);
  for (const server of [first, second]) {
    const response = await fetch(`http://127.0.0.1:${server.httpServer.address().port}/`, { signal: AbortSignal.timeout(5000) });
    assert.equal(response.status, 200);
    await response.text();
  }
});
