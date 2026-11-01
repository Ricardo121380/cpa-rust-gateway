// Mechanical invariants for the console bundle. The gateway embeds exactly four files and serves them
// under a strict CSP (crates/gateway-http-actix/src/management_ui_resources.rs), so violations here
// would surface as a blank page or a failed cargo build, not as a lint warning.
import { execSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const fail = [];
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));

// 1. Contract types are current.
try { execSync("node scripts/gen-types.mjs --check", { cwd: root, stdio: "pipe" }); } catch (e) { fail.push(String(e.stderr || e.stdout).trim()); }

// 2. Source rules: no browser storage, no raw fetch outside the client, no innerHTML injection, no <style> injection.
const src = walk(join(root, "src")).filter((f) => /\.(t|j)sx?$/.test(f) && !f.endsWith(".d.ts") && !f.includes("/api/mock/"));
for (const f of src) {
  const text = readFileSync(f, "utf8"), rel = relative(root, f);
  if (/\b(localStorage|sessionStorage|indexedDB)\b/.test(text)) fail.push(`${rel}: browser storage is not allowed (credentials and state stay in memory)`);
  if (/dangerouslySetInnerHTML/.test(text)) fail.push(`${rel}: dangerouslySetInnerHTML is not allowed`);
  if (/createElement\(\s*["']style["']\s*\)/.test(text)) fail.push(`${rel}: runtime <style> injection violates style-src 'self'`);
  if (/\bfetch\(/.test(text) && !rel.startsWith("src/api/client.ts")) fail.push(`${rel}: call the management API through src/api/client.ts`);
}

// 3. Bundle shape: exactly the four files the gateway embeds, nothing inlined, no external origins.
const dist = join(root, "dist");
const want = ["assets/index.css", "assets/main.js", "assets/vendor.js", "index.html"];
let files = [];
try { files = walk(dist).map((f) => relative(dist, f)).sort(); } catch { fail.push("dist/ missing: run npm run build first"); }
if (files.length && JSON.stringify(files) !== JSON.stringify(want)) fail.push(`dist must be exactly ${want.join(", ")}; found ${files.join(", ")}`);
if (files.includes("index.html")) {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  if (/<script(?![^>]*\bsrc=)[^>]*>/.test(html)) fail.push("index.html: inline <script> violates script-src 'self'");
  if (/<style[\s>]/.test(html) || /\sstyle=/.test(html)) fail.push("index.html: inline styles violate style-src 'self'");
}
for (const f of files) {
  const text = readFileSync(join(dist, f), "utf8");
  const ext = text.match(/(?:src|href)\s*=\s*["']https?:\/\/[^"']+/g) || [];
  const css = f.endsWith(".css") ? text.match(/url\(\s*["']?https?:\/\/[^)]+/g) || [] : [];
  const imp = f.endsWith(".css") ? text.match(/@import\s+(url\()?["']?https?:/g) || [] : [];
  for (const m of [...ext, ...css, ...imp]) fail.push(`${f}: external resource ${m.slice(0, 80)} is blocked by the CSP`);
  if (/VITE_CONSOLE_MOCK|mockFetch|management_login_invalid_credentials\b.*mock/.test(text) && /mock\/server/.test(text)) fail.push(`${f}: mock transport leaked into the production bundle`);
}
if (files.includes("assets/main.js") && /api\/mock|mockFetch/.test(readFileSync(join(dist, "assets/main.js"), "utf8"))) fail.push("assets/main.js: mock transport leaked into the production bundle");

if (fail.length) { console.error("✗ console check failed:\n  - " + fail.join("\n  - ")); process.exit(1); }
console.log(`✓ console check passed (${src.length} source files, ${files.length} dist files)`);
