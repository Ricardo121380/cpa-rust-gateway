import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Reproducible-build contract (docs/08 §3): fixed file names, no content hashes,
// no sourcemaps, no timestamps. The emitted file set must match the embedding
// manifest in gateway-http-actix exactly (cutover happens at FE-1 exit).
export default defineConfig({
  // Relative asset paths: the SPA is served under the management listener's
  // /admin-ui/ prefix after cutover; hash routing needs no server fallback.
  base: "./",
  plugins: [react()],
  build: {
    sourcemap: false,
    minify: "esbuild",
    modulePreload: { polyfill: false },
    rollupOptions: {
      output: {
        entryFileNames: "assets/main.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: "assets/[name][extname]",
        manualChunks: {
          vendor: [
            "react",
            "react-dom",
            "react-router-dom",
            "@tanstack/react-query",
            "zustand",
          ],
        },
      },
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    env: { VITE_PRISM_FIXTURES: "1" },
  },
});
