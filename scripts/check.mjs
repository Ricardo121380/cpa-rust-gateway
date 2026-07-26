#!/usr/bin/env node
// Prism SPA invariant checks (gateway docs/08 §5.2):
//   1. no browser storage APIs anywhere in src (C6)
//   2. no raw fetch( outside the generated client (C5)
//   3. generated client is fresh vs contracts/management-v1.json
//   4. dist file set matches the embedding manifest exactly (C3)
//   5. dist/index.html carries CSP meta and no inline script/style (C4)
//   6. --double-build: two builds are byte-identical (C10)
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const SRC = join(ROOT, "src");
const DIST = join(ROOT, "dist");

const EXPECTED_DIST_FILES = [
  "index.html",
  "assets/index.css",
  "assets/main.js",
  "assets/vendor.js",
];

const failures = [];

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walk(path));
    } else {
      out.push(path);
    }
  }
  return out;
}

// 1 + 2: source scans
const STORAGE_PATTERN = /\b(localStorage|sessionStorage|indexedDB|document\.cookie)\b/u;
const FETCH_PATTERN = /\bfetch\s*\(/u;
for (const file of walk(SRC)) {
  if (!/\.(ts|tsx|css)$/u.test(file)) continue;
  const rel = relative(ROOT, file);
  const text = readFileSync(file, "utf8");
  if (STORAGE_PATTERN.test(text)) {
    failures.push(`${rel}: browser storage API is banned (C6)`);
  }
  const isGenerated = rel.startsWith("src/generated/");
  if (!isGenerated && FETCH_PATTERN.test(text)) {
    failures.push(`${rel}: raw fetch() outside the generated client is banned (C5)`);
  }
  // Inline style attributes are blocked by the shipped CSP (style-src 'self'
  // with no inline exemption) — use classes, or SVG presentation attributes
  // for dynamic geometry.
  if (/\bstyle=\{\{/u.test(text)) {
    failures.push(`${rel}: inline style attribute is blocked by the production CSP`);
  }
  // Password-typed fields summon Safari's strong-password popover and
  // password-manager widgets, which cover the input and swallow paste. The
  // unlock secrets are machine keys: they use masked text inputs instead.
  const codeLines = text
    .split("\n")
    .filter((line) => !/^\s*(?:\/\/|\*|\/\*)/u.test(line))
    .join("\n");
  if (rel.startsWith("src/features/unlock/") && /type="password"/u.test(codeLines)) {
    failures.push(`${rel}: type="password" breaks paste on the unlock screen (use SecretField)`);
  }
}

function distFileList() {
  return walk(DIST)
    .map((file) => relative(DIST, file))
    .sort();
}

function hashDist() {
  const hash = createHash("sha256");
  for (const rel of distFileList()) {
    hash.update(rel);
    hash.update(readFileSync(join(DIST, rel)));
  }
  return hash.digest("hex");
}

function build() {
  execSync("npm run build:bundle", { cwd: ROOT, stdio: "pipe" });
}

// generated client freshness: the client must match contracts/management-v1.json
try {
  execSync("node scripts/generate-client.mjs --check", { cwd: ROOT, stdio: "pipe" });
} catch {
  failures.push(
    "src/generated/management-client.ts is stale vs contracts/management-v1.json — run npm run sync-contract (or npm run generate)",
  );
}

// 3 + 4 (+5): build artifacts
try {
  statSync(DIST);
} catch {
  build();
}

const actual = distFileList();
const expected = [...EXPECTED_DIST_FILES].sort();
if (JSON.stringify(actual) !== JSON.stringify(expected)) {
  failures.push(
    `dist file set mismatch (C3)\n  expected: ${expected.join(", ")}\n  actual:   ${actual.join(", ")}`,
  );
}

const html = readFileSync(join(DIST, "index.html"), "utf8");
if (!html.includes("Content-Security-Policy")) {
  failures.push("dist/index.html: missing CSP meta (C4)");
}
if (/<script(?![^>]*\bsrc=)/u.test(html)) {
  failures.push("dist/index.html: inline script detected (C4)");
}
if (/<style/u.test(html) || /style="/u.test(html)) {
  failures.push("dist/index.html: inline style detected (C4)");
}
if (!html.includes("style-src 'self';")) {
  failures.push("dist/index.html: production CSP must keep style-src 'self' without inline exemption");
}

if (process.argv.includes("--double-build")) {
  const first = hashDist();
  rmSync(DIST, { recursive: true });
  build();
  const second = hashDist();
  if (first !== second) {
    failures.push(`double build differs (C10): ${first} != ${second}`);
  }
}

if (failures.length > 0) {
  console.error("check-prism-spa: FAILED");
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}
console.log("check-prism-spa: OK");
