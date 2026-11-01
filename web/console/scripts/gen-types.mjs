// Generate TypeScript types and a header-requirement table from the gateway's management contract
// (docs/openapi/management-v1.json is the single source of truth). `--check` fails when either output is stale.
import { readFileSync, writeFileSync } from "node:fs";
import openapiTS, { astToString } from "openapi-typescript";

const contract = JSON.parse(readFileSync(new URL("../../../docs/openapi/management-v1.json", import.meta.url), "utf8"));
const outputs = [];

const ast = await openapiTS(contract);
outputs.push(["../src/api/schema.d.ts", "// Generated from docs/openapi/management-v1.json by scripts/gen-types.mjs. Do not edit.\n" + astToString(ast)]);

// Which operations take X-Config-Version / If-Match, so the client attaches them mechanically.
const resolve = (p) => (p.$ref ? contract.components.parameters[p.$ref.split("/").pop()] : p);
const table = {};
for (const [path, item] of Object.entries(contract.paths)) {
  for (const method of ["get", "post", "put", "patch", "delete"]) {
    const op = item[method];
    if (!op) continue;
    const headers = [...(item.parameters || []), ...(op.parameters || [])].map(resolve).filter((p) => p.in === "header");
    const cv = headers.find((h) => h.name === "X-Config-Version");
    const im = headers.find((h) => h.name === "If-Match");
    table[`${method.toUpperCase()} ${path}`] = { op: op.operationId, cv: cv ? (cv.required ? 2 : 1) : 0, ifMatch: im ? 1 : 0 };
  }
}
outputs.push(["../src/api/ops.gen.ts", "// Generated from docs/openapi/management-v1.json by scripts/gen-types.mjs. Do not edit.\n// cv: 0 = no X-Config-Version, 1 = optional, 2 = required. ifMatch: 1 = If-Match required.\nexport const OPS: Record<string, { op: string; cv: 0 | 1 | 2; ifMatch: 0 | 1 }> = " + JSON.stringify(table, null, 1) + ";\n"]);

let stale = false;
for (const [rel, text] of outputs) {
  const url = new URL(rel, import.meta.url);
  if (process.argv.includes("--check")) {
    let cur = "";
    try { cur = readFileSync(url, "utf8"); } catch { /* missing counts as stale */ }
    if (cur !== text) { console.error(`${rel} is stale; run npm run gen:types`); stale = true; }
  } else {
    writeFileSync(url, text);
    console.log("wrote", rel.replace("../", ""), (text.length / 1024).toFixed(0) + " KB");
  }
}
if (stale) process.exit(1);
