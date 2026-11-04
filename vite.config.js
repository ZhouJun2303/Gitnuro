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
    // Lab HTTP lives on 1429 only for `pnpm dev` / `pnpm test:ui` (AWEGIT_LAB set).
    // Desktop `pnpm start` must not proxy here or Vite logs ECONNREFUSED on /invoke.
    proxy: process.env.AWEGIT_LAB
      ? {
          "/__lab": {
            target: "http://127.0.0.1:1429",
            rewrite: (path) => path.replace(/^\/__lab/, "") || "/",
          },
        }
      : undefined,
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
