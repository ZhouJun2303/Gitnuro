import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;
const port = Number(process.env.AWEGIT_VITE_PORT || 1420);

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port,
    strictPort: true,
    host: host || "127.0.0.1",
    // Browser lab posts to `/__lab/*`; awegit-lab listens on 1429 (`pnpm dev` / `pnpm test:ui`).
    // Keep the proxy even under `pnpm start` so a browser tab can reuse this Vite once lab is up.
    proxy: {
      "/__lab": {
        target: "http://127.0.0.1:1429",
        rewrite: (path) => path.replace(/^\/__lab/, "") || "/",
        configure: (proxy) => {
          // http-proxy types ship without EventEmitter helpers under check.
          /** @type {any} */
          const target = proxy;
          target.on("error", (/** @type {unknown} */ _err, /** @type {unknown} */ _req, /** @type {any} */ res) => {
            if (!res || res.headersSent || typeof res.writeHead !== "function") return;
            res.writeHead(502, { "Content-Type": "application/json; charset=utf-8" });
            res.end(JSON.stringify({ ok: false, error: "试验台未启动，请运行 pnpm dev" }));
          });
        },
      },
    },
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**", "**/.lab/**"],
    },
  },
}));
