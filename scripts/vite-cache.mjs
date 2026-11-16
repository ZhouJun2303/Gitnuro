/** @returns {import("vite").Plugin} */
export function isolatedViteCache() {
  const temporary = `temporary-${globalThis.crypto.randomUUID()}`;
  return {
    name: "awegit:isolated-vite-cache",
    config(config, { command }) {
      if (config.cacheDir) return;
      // The config hook sees CLI/API port overrides after Vite merges them.
      const port = config.server?.port ?? 1420;
      const scope = command === "build"
        ? "build"
        : port === 0 || config.server?.middlewareMode ? temporary : `dev-${port}`;
      // Client and SSR optimizers must both stay inside this server's cache.
      return { cacheDir: `node_modules/.vite/awegit/${scope}` };
    },
  };
}
