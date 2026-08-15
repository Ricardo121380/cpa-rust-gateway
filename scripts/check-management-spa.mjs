#!/usr/bin/env node

// Compatibility entry point for the repository-wide gate.
//
// The management SPA moved from web/admin-ui to web/prism. Keep this root
// command stable for historical gate scripts, but make the current Prism
// checker authoritative instead of carrying a second, stale frontend model.

import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const uiRoot = path.join(root, "web/prism");
const specificationPath = path.join(root, "docs/openapi/management-v1.json");
const vendoredContractPath = path.join(uiRoot, "contracts/management-v1.json");
const generatedClientPath = path.join(uiRoot, "src/generated/management-client.ts");
const httpMethods = new Set(["get", "post", "patch", "delete"]);

function assert(condition, message) {
  if (!condition) {
    throw new Error(`management-spa check: ${message}`);
  }
}

const specificationText = await readFile(specificationPath, "utf8");
const vendoredContractText = await readFile(vendoredContractPath, "utf8");
assert(
  specificationText === vendoredContractText,
  "Prism vendored contract differs from docs/openapi/management-v1.json — run npm --prefix web/prism run sync-contract",
);

const specification = JSON.parse(specificationText);
assert(
  specification?.openapi === "3.1.0" && specification?.["x-contract-status"] === "contract_only",
  "management contract must remain OpenAPI 3.1 contract_only",
);

const operationIds = [];
for (const [route, pathItem] of Object.entries(specification.paths ?? {})) {
  assert(route.startsWith("/admin/"), `non-admin route ${route}`);
  for (const [method, operation] of Object.entries(pathItem)) {
    if (!httpMethods.has(method)) continue;
    assert(
      typeof operation?.operationId === "string" && /^[A-Za-z][A-Za-z0-9]*$/u.test(operation.operationId),
      `invalid operationId at ${method.toUpperCase()} ${route}`,
    );
    operationIds.push(operation.operationId);
  }
}
assert(operationIds.length > 0, "contract operations are incomplete");
assert(new Set(operationIds).size === operationIds.length, "contract operation IDs are not unique");

const generatedClient = await readFile(generatedClientPath, "utf8");
for (const operationId of operationIds) {
  assert(
    new RegExp(`\\n  ${operationId}\\(request: ManagementRequest = \\{\\}\\): Promise<Response>`, "u").test(
      generatedClient,
    ),
    `generated client is missing wrapper ${operationId}`,
  );
}

// Prism's checker owns source policy, CSP, generated-client freshness and the
// reproducible two-build assertion. Running its full variant here keeps the
// legacy root gate and the current frontend gate on one implementation.
execFileSync("npm", ["--prefix", uiRoot, "run", "check:full"], {
  cwd: root,
  stdio: "inherit",
});

console.log(`management-spa: ok (${operationIds.length} generated operations, Prism double-build check)`);
