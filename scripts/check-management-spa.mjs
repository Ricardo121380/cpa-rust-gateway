#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import nodeAssert from "node:assert/strict";
import { readdir, readFile, stat } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const uiRoot = path.join(root, "web/admin-ui");
const distRoot = path.join(uiRoot, "dist");
const specificationPath = path.join(root, "docs/openapi/management-v1.json");
const generatedClientPath = path.join(uiRoot, "src/generated/management-client.ts");
const applicationPath = path.join(uiRoot, "src/main.ts");

function run(command, commandArguments) {
  execFileSync(command, commandArguments, { cwd: root, stdio: "inherit" });
}

async function digestTree(directory, relative = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const result = new Map();
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const childRelative = path.posix.join(relative, entry.name);
    const childPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      for (const [key, value] of await digestTree(childPath, childRelative)) {
        result.set(key, value);
      }
    } else if (entry.isFile()) {
      result.set(childRelative, createHash("sha256").update(await readFile(childPath)).digest("hex"));
    }
  }
  return result;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(`management-spa check: ${message}`);
  }
}

const specification = JSON.parse(await readFile(specificationPath, "utf8"));
const operationIds = Object.values(specification.paths)
  .flatMap((pathItem) => Object.entries(pathItem))
  .filter(([method]) => ["get", "post", "patch", "delete"].includes(method))
  .map(([, operation]) => operation.operationId);
assert(operationIds.length > 0 && operationIds.every((operationId) => typeof operationId === "string"), "contract operations are incomplete");

run("node", ["scripts/generate-management-client.mjs", "--check"]);
run("scripts/build-management-spa.sh", []);
const firstBuild = await digestTree(distRoot);
run("scripts/build-management-spa.sh", []);
const secondBuild = await digestTree(distRoot);
assert(JSON.stringify([...firstBuild]) === JSON.stringify([...secondBuild]), "two clean static builds differ");

const expectedAssets = ["index.html", "assets/main.js", "assets/generated/management-client.js", "assets/styles.css"];
for (const asset of expectedAssets) {
  assert((await stat(path.join(distRoot, asset))).isFile(), `missing built asset ${asset}`);
}

const generatedClient = await readFile(generatedClientPath, "utf8");
for (const operationId of operationIds) {
  assert(generatedClient.includes(`  ${operationId}(request: ManagementRequest = {})`), `missing generated wrapper ${operationId}`);
}
assert(!/localStorage|sessionStorage|indexedDB|document\.cookie/u.test(generatedClient), "generated client persists credential material");
assert(generatedClient.includes('credentials: "same-origin"'), "generated client does not keep management traffic same-origin");
assert(generatedClient.includes("redirect: \"error\""), "generated client permits redirect following");

const { ManagementApi } = await import(
  `${pathToFileURL(path.join(distRoot, "assets/generated/management-client.js")).href}?check=${Date.now()}`,
);
const observedRequests = [];
const api = new ManagementApi({
  managementKey: () => "mgmt_test-key-without-persistence",
  csrfToken: () => "csrf_test-token-without-persistence",
  fetch: async (input, init) => {
    observedRequests.push({ input, init });
    return new Response(null, { status: 204 });
  },
});
await api.getUpstream({
  path: { upstream_id: "upstream/a" },
  headers: { "X-Config-Version": "draft-1" },
});
assert(observedRequests.length === 1, "safe generated GET did not invoke the supplied fetch exactly once");
assert(observedRequests[0].input === "/admin/upstreams/upstream%2Fa", "generated path is not relative and encoded");
assert(observedRequests[0].init.headers.get("X-Management-Key") === "mgmt_test-key-without-persistence", "client did not own the management key header");
assert(observedRequests[0].init.headers.get("X-Management-CSRF-Token") === null, "safe GET received a CSRF header");

await api.createUpstream({
  headers: { "X-Config-Version": "draft-1", "If-Match": "revision-1" },
  body: { name: "synthetic" },
});
assert(observedRequests.length === 2, "generated mutation did not invoke the supplied fetch exactly once");
assert(observedRequests[1].input === "/admin/upstreams", "generated mutation uses an unexpected path");
assert(observedRequests[1].init.headers.get("X-Management-CSRF-Token") === "csrf_test-token-without-persistence", "unsafe operation did not require the client-owned CSRF header");
assert(observedRequests[1].init.headers.get("Content-Type") === "application/json", "JSON operation lacks its exact content type");
assert(observedRequests[1].init.body === '{"name":"synthetic"}', "JSON body was not deterministically serialized");

await api.previewRestore({ body: new Uint8Array([1, 2, 3]) });
assert(observedRequests.length === 3, "generated binary operation did not invoke the supplied fetch exactly once");
assert(observedRequests[2].init.headers.get("Content-Type") === "application/octet-stream", "binary contract operation lacks its exact content type");
assert(observedRequests[2].init.body instanceof Uint8Array, "binary contract body was unexpectedly serialized");

const rejectedRequests = observedRequests.length;
await nodeAssert.rejects(
  api.createUpstream({ headers: { "X-Config-Version": "draft-1" }, body: {} }),
  /missing required management header input/u,
);
await nodeAssert.rejects(
  api.getUpstream({
    path: { upstream_id: "upstream-a" },
    headers: { "X-Config-Version": "draft-1", "X-Management-Key": "caller-supplied" },
  }),
  /undeclared management header input/u,
);
const noCsrfApi = new ManagementApi({
  managementKey: () => "mgmt_test-key-without-persistence",
  fetch: async () => new Response(null, { status: 204 }),
});
await nodeAssert.rejects(noCsrfApi.createConfigVersion({ body: { name: "synthetic" } }), /X-Management-CSRF-Token/u);
assert(observedRequests.length === rejectedRequests, "invalid generated calls reached fetch");

const application = await readFile(applicationPath, "utf8");
assert(!/\bfetch\s*\(|new ManagementApi/u.test(application), "P10-03 shell sends a management request");
const html = await readFile(path.join(distRoot, "index.html"), "utf8");
assert(html.includes("Content-Security-Policy"), "static document has no CSP");
assert(!/<script(?![^>]*\bsrc=)/u.test(html), "static document contains inline script");

console.log(`management-spa: ok (${operationIds.length} generated operations, reproducible static build)`);
