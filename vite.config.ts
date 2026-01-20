import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
const rootDir = fileURLToPath(new URL(".", import.meta.url));
const testPatternTs = resolve(rootDir, "src/**/*.test.ts").replace(/\\/g, "/");
const testPatternTsx = resolve(rootDir, "src/**/*.test.tsx").replace(/\\/g, "/");

export default defineConfig(async () => ({
  root: rootDir,
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    include: [testPatternTs, testPatternTsx],
    setupFiles: resolve(rootDir, "src/setupTests.ts"),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
