import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { isolatedViteCache } from "./scripts/vite-cache.mjs";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;
const port = Number(process.env.AWEGIT_VITE_PORT || 1420);
const clientDependencies = [
  "@tauri-apps/api/core",
  "@tauri-apps/api/dpi",
  "@tauri-apps/api/event",
  "@tauri-apps/api/window",
  "@tauri-apps/plugin-opener",
  "highlight.js/lib/core",
  "highlight.js/lib/languages/bash",
  "highlight.js/lib/languages/c",
  "highlight.js/lib/languages/cpp",
  "highlight.js/lib/languages/csharp",
  "highlight.js/lib/languages/dart",
  "highlight.js/lib/languages/go",
  "highlight.js/lib/languages/ini",
  "highlight.js/lib/languages/java",
  "highlight.js/lib/languages/json",
  "highlight.js/lib/languages/kotlin",
  "highlight.js/lib/languages/lua",
  "highlight.js/lib/languages/objectivec",
  "highlight.js/lib/languages/php",
  "highlight.js/lib/languages/python",
  "highlight.js/lib/languages/ruby",
  "highlight.js/lib/languages/rust",
  "highlight.js/lib/languages/scala",
  "highlight.js/lib/languages/sql",
  "highlight.js/lib/languages/swift",
  "highlight.js/lib/languages/typescript",
  "highlight.js/lib/languages/xml",
  "highlight.js/lib/languages/yaml",
];

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [isolatedViteCache(), sveltekit()],
  optimizeDeps: {
    include: clientDependencies,
  },

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
      // Generated Rust artifacts and the local package store can contain tens
      // of thousands of files; watching them delays startup and wastes memory.
      ignored: ["**/src-tauri/**", "**/target/**", "**/.pnpm-store/**", "**/.lab/**"],
    },
  },
}));
