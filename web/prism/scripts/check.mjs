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
  // A "proposed endpoint" side channel once carried a whole analytics shape the
  // backend never implemented. It was gated on dev fixtures, so ~3500 lines of
  // page code — the usage page, the monitoring page, half of overview, six
  // chart components — rendered a "contract pending" empty state in every
  // production build and nobody noticed for months.
  //
  // The contract is the only source of endpoints. When a shape is missing, the
  // route is docs/change-requests/ plus an honest empty state, not a second
  // client that answers only in dev.
  if (/["']\.{1,2}\/.*api\/proposed|from ["'][^"']*api\/proposed/u.test(text)) {
    failures.push(
      `${rel}: a proposed-endpoint channel is banned — the contract is the only source of endpoints (see docs/08 §B5)`,
    );
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
  // --ink-3 measures 3.26:1 (light) / 3.56:1 (dark) — below AA. It is reserved
  // for non-text marks: chart grid lines and axis ticks, the uppercase 11px
  // labels that repeat the data below them, status dots. A rule that puts it on
  // a `color:` alongside a font-size in body range is putting a sentence in a
  // sub-AA colour, which is how .card-note shipped at 3.3:1. Raising the token
  // is not the fix: any AA-clean value collapses into --ink-2.
  if (rel.endsWith(".css")) {
    const blocks = text.split("}");
    for (const block of blocks) {
      if (!/color:\s*var\(--ink-3\)/u.test(block)) continue;
      const selector = (block.split("{")[0] ?? "").trim().split("\n").pop() ?? "?";
      const size = /font-size:\s*(\d+(?:\.\d+)?)px/u.exec(block);
      if (size !== null && Number(size[1]) >= 12) {
        failures.push(
          `${rel}: ${selector} puts ${size[1]}px body text in --ink-3 (below AA) — use --ink-2`,
        );
        continue;
      }
      // The hole this closes: a block with NO local font-size inherits one, and
      // an inherited size is not statically knowable — so the rule above simply
      // never fired for it. Rather than resolve the cascade, require the block
      // to make its own case: either a sub-12px size right here, or an explicit
      // `ink-3:` note saying why this is a non-text mark.
      if (size === null && !/ink-3:/u.test(block)) {
        failures.push(
          `${rel}: ${selector} sets --ink-3 with no local font-size — its size is inherited and` +
            ` cannot be checked. Declare the font-size here, or annotate the block with` +
            ` "/* ink-3: <why this is a non-text mark> */".`,
        );
      }
    }
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

// Contract regression: PUT must remain representable by the generated client.
// The routing-price policy setter uses PUT; an HTTP-method allow-list
// regression would otherwise silently drop it while freshness still passes.
const generatedClient = readFileSync(join(ROOT, "src/generated/management-client.ts"), "utf8");
if (!/export type ManagementHttpMethod = [^;]*"PUT"/u.test(generatedClient)) {
  failures.push("generated management client must support PUT operations");
}
if (!/"setRoutingPricePolicy":\s*\{\s*"method":\s*"PUT"/su.test(generatedClient)) {
  failures.push("generated management client is missing setRoutingPricePolicy PUT operation");
}

// 3 + 4 (+5): build artifacts
// A pre-existing dist is reused so the plain `check` stays fast. Under
// --double-build that shortcut is wrong twice over: the artifact assertions
// below would inspect a stale build, and the reproducibility comparison would
// pit that stale build against a fresh one and fail on a difference that is not
// in this tree. So force one fresh build up front and let both halves use it.
const DOUBLE = process.argv.includes("--double-build");
if (DOUBLE) {
  rmSync(DIST, { recursive: true, force: true });
}
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

if (DOUBLE) {
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
