import { defineConfig, type Plugin } from "vitest/config";
import react from "@vitejs/plugin-react";

// The shipped policy is strict: `style-src 'self'` with no inline exemption,
// which is why no component uses a style attribute (dynamic geometry goes
// through SVG presentation attributes). Vite's dev server injects styles as
// inline <style> tags, so DEV gets a policy that permits exactly that and
// nothing more — production keeps the strict one.
// frame-ancestors is ignored when delivered in a <meta> element (the browser
// says so in the console). The gateway sends the real policy as a header on
// every embedded asset, which is where frame-ancestors takes effect — so the
// meta policy carries only the directives a meta CSP can actually enforce.
const PROD_CSP =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; " +
  "connect-src 'self'; form-action 'none'";
const DEV_CSP =
  "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; " +
  "style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ws:; " +
  "form-action 'none'";

function cspPlugin(): Plugin {
  return {
    name: "prism-csp",
    transformIndexHtml: {
      order: "pre",
      handler(html, context) {
        const policy = context.server === undefined ? PROD_CSP : DEV_CSP;
        return html.replace(
          "<!--CSP-->",
          `<meta http-equiv="Content-Security-Policy" content="${policy}" />`,
        );
      },
    },
  };
}

// Fixed on-disk names are required by the Rust embedder. Version every URL,
// including the entry's vendor import, so prior releases cannot share modules.
function assetRevisionPlugin(): Plugin {
  return {
    name: "prism-asset-revision",
    enforce: "post",
    async generateBundle(_options, bundle) {
      const contents: string[] = [];
      for (const name of Object.keys(bundle).sort()) {
        const asset = bundle[name]!;
        const source = asset.type === "chunk" ? asset.code : asset.source;
        contents.push(name, typeof source === "string" ? source : new TextDecoder().decode(source));
      }
      const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(contents)));
      const revision = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("").slice(0, 24);
      for (const asset of Object.values(bundle)) {
        if (asset.type === "chunk") {
          asset.code = asset.code.replaceAll('"./vendor.js"', `"./vendor.js?v=${revision}"`);
        } else if (asset.fileName === "index.html") {
          asset.source = String(asset.source).replace(
            /\.\/assets\/(main\.js|vendor\.js|index\.css)/gu,
            `./assets/$1?v=${revision}`,
          );
        }
      }
    },
  };
}

// Reproducible-build contract (docs/08 §3): fixed file names, no content hashes,
// no sourcemaps, no timestamps. The emitted file set must match the embedding
// manifest in gateway-http-actix exactly (cutover happens at FE-1 exit).
export default defineConfig({
  // Relative asset paths: the SPA is served under the management listener's
  // /admin-ui/ prefix after cutover; hash routing needs no server fallback.
  base: "./",
  plugins: [react(), cspPlugin(), assetRevisionPlugin()],
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
