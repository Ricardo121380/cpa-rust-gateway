import path from "node:path";
import { loadEnv } from "vite";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
const __dirname_ = path.dirname(new URL(import.meta.url).pathname);

// The gateway embeds exactly four files (crates/gateway-http-actix/src/management_ui_resources.rs):
// index.html, assets/index.css, assets/main.js, assets/vendor.js. Names are fixed and unhashed,
// and the CSP allows only 'self' scripts and styles, so nothing may be inlined.
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const target = env.CONSOLE_GATEWAY || "http://127.0.0.1:8318";
  return {
    base: "./",
    plugins: [react(), tailwindcss()],
    resolve: { alias: { "@": path.resolve(__dirname_, "src"), "next/link": path.resolve(__dirname_, "src/lib/shims/next-link.tsx") } },
    build: {
      assetsInlineLimit: 0,
      cssCodeSplit: false,
      modulePreload: { polyfill: false },
      chunkSizeWarningLimit: 4000,
      target: "es2022",
      rollupOptions: {
        output: {
          entryFileNames: "assets/main.js",
          chunkFileNames: "assets/[name].js",
          assetFileNames: (a) => (a.names?.[0]?.endsWith(".css") ? "assets/index.css" : "assets/[name][extname]"),
          // Third-party code goes to vendor.js; everything else stays in main.js.
          manualChunks: (id) => (id.includes("node_modules") ? "vendor" : undefined),
        },
      },
    },
    server: {
      // The management listener admits exactly one browser Origin and rejects others with 404,
      // so the dev proxy rewrites Origin to the listener's own address.
      proxy: { "/admin": { target, changeOrigin: true, headers: { Origin: target } } },
    },
    test: { environment: "node" },
  };
});
