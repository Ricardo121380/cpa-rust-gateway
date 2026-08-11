#!/usr/bin/env node
// Copy the management OpenAPI contract from the repository's own
// docs/openapi (the single source of truth since the frontend was merged in
// at web/prism), then regenerate the API client and verify freshness.
//
//   node scripts/sync-contract.mjs            # copy + regenerate
//   GATEWAY_REPO=/path/to/repo node scripts/sync-contract.mjs   # override
import { copyFile, readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const gatewayRepo =
  process.env["GATEWAY_REPO"] ?? path.resolve(root, "../..");
const source = path.join(gatewayRepo, "docs/openapi/management-v1.json");
const target = path.join(root, "contracts/management-v1.json");

const before = await readFile(target, "utf8").catch(() => "");
const incoming = await readFile(source, "utf8").catch(() => {
  console.error(`sync-contract: cannot read ${source}`);
  console.error("expected the repository root two levels above web/prism");
  process.exit(1);
});

if (incoming === before) {
  console.log("sync-contract: contract unchanged");
} else {
  await copyFile(source, target);
  console.log(`sync-contract: updated contracts/management-v1.json from ${source}`);
}

execFileSync("node", [path.join(root, "scripts/generate-client.mjs")], { stdio: "inherit" });
console.log("sync-contract: client regenerated — review src/dev/fixtures.ts against contract changes");
