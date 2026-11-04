import { existsSync, readFileSync } from "node:fs";
import { defineConfig } from "vitest/config";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;
// The browser/jsdom fallback state reports this version, so it is derived from
// package.json here instead of being hand-edited on every release.
const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as { version: string };

export default defineConfig({
  define: { __APP_VERSION__: JSON.stringify(version) },
  plugins: [react(), tailwindcss(), {
    name: "find-model-notices",
    generateBundle() {
      // Shipped beside the model; absent (with the model) on a checkout that has not fetched it.
      for (const name of ["LICENSE", "NOTICE"]) {
        const file = new URL("./src/find/model/" + name, import.meta.url);
        if (existsSync(file)) this.emitFile({ type: "asset", fileName: "assets/find-model." + name, source: readFileSync(file) });
      }
    },
  }],
  test: { environment: "jsdom", setupFiles: ["./src/test-setup.ts"] },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
