#!/usr/bin/env node

// Local deterministic browser-fixture server for the P10-04 SPA evidence run.
// It serves only built static assets and synthetic, value-free management responses; it has no
// Provider transport, persistence, credential material, network forwarding, or external egress.

import http from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = path.join(root, "web/admin-ui/dist");
const port = Number.parseInt(process.env.P10_BROWSER_PORT ?? "4179", 10);
const fixtureSession = "browser-fixture";
const managementKey = ["mgmt", fixtureSession, "key"].join("_");
const csrfToken = ["csrf", fixtureSession].join("-");
let revision = 0;

function responseJson(response, status, value, nextRevision) {
  const headers = { "Content-Type": "application/json", "Cache-Control": "no-store" };
  if (nextRevision !== undefined) {
    headers.ETag = `"rev-${nextRevision}"`;
  }
  response.writeHead(status, headers);
  response.end(JSON.stringify(value));
}

function denied(response) {
  response.writeHead(404, { "Content-Type": "application/json", "Cache-Control": "no-store" });
  response.end(JSON.stringify({ error: { code: "management_not_found", message: "Not found" } }));
}

async function managementRequest(request, response, pathname) {
  if (request.headers["x-management-key"] !== managementKey || request.headers["x-config-version"] !== "draft-p10") {
    denied(response);
    return;
  }
  const unsafe = request.method !== "GET";
  if (unsafe && request.headers["x-management-csrf-token"] !== csrfToken) {
    denied(response);
    return;
  }
  const requiresRevision = pathname.endsWith("discover-apply") || ["POST", "PATCH", "DELETE"].includes(request.method) && !pathname.includes("/oauth/") && !pathname.endsWith("/test") && !pathname.endsWith("discover-preview");
  if (requiresRevision && request.headers["if-match"] !== `rev-${revision}`) {
    responseJson(response, 409, { error: { code: "management_revision_conflict", message: "Management configuration changed" } });
    return;
  }
  if (pathname === "/admin/upstreams" && request.method === "GET") {
    responseJson(response, 200, []);
    return;
  }
  if (pathname === "/admin/upstreams" && request.method === "POST") {
    revision += 1;
    responseJson(response, 201, { id: "provider-a", name: "Provider A", kind: "openai-compatible", enabled: true, tags: [], egress_policy_id: null }, revision);
    return;
  }
  if (pathname.endsWith("/credential-bindings") && request.method === "POST") {
    let body = "";
    for await (const chunk of request) {
      body += chunk;
      if (body.length > 70 * 1024) {
        responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
        return;
      }
    }
    try {
      const input = JSON.parse(body);
      const expectedKeys = ["concurrency", "credential_id", "enabled", "priority", "weight"];
      if (input === null || typeof input !== "object" || Object.keys(input).sort().join(",") !== expectedKeys.join(",")) {
        throw new Error("binding input is not contract-owned");
      }
    } catch {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return;
    }
    revision += 1;
    responseJson(response, 201, { endpoint_id: "endpoint-b", upstream_id: "provider-a", credential_id: "provider-a-key-1", enabled: true, priority: 0, weight: 100, concurrency: 1 }, revision);
    return;
  }
  if (pathname.endsWith("/test") && request.method === "POST") {
    responseJson(response, 200, { outcome: "pass", status_class: "2xx", canonical_lifecycle: true });
    return;
  }
  if (pathname.endsWith("discover-preview") && request.method === "POST") {
    responseJson(response, 200, { added: 3, removed: 1, unchanged: 8 });
    return;
  }
  if (pathname.endsWith("discover-apply") && request.method === "POST") {
    revision += 1;
    responseJson(response, 200, { added: 3, removed: 1, unchanged: 8 }, revision);
    return;
  }
  if (pathname.endsWith("/oauth/start") && request.method === "POST") {
    responseJson(response, 202, { credential_id: "provider-a-key-1", state: "pending", expires_at_ms: 99 });
    return;
  }
  if (pathname.endsWith("/oauth/status") && request.method === "GET") {
    responseJson(response, 200, { credential_id: "provider-a-key-1", state: "pending", expires_at_ms: 99 });
    return;
  }
  if (pathname.endsWith("/oauth/cancel") && request.method === "POST") {
    response.writeHead(204, { "Cache-Control": "no-store" });
    response.end();
    return;
  }
  denied(response);
}

async function staticAsset(response, pathname) {
  const relative = pathname === "/" ? "index.html" : pathname.replace(/^\//u, "");
  const candidate = path.resolve(assetRoot, relative);
  if (!candidate.startsWith(`${assetRoot}${path.sep}`) && candidate !== path.join(assetRoot, "index.html")) {
    denied(response);
    return;
  }
  try {
    const contents = await readFile(candidate);
    const contentType = candidate.endsWith(".html") ? "text/html; charset=utf-8" : candidate.endsWith(".css") ? "text/css; charset=utf-8" : "application/javascript; charset=utf-8";
    response.writeHead(200, { "Content-Type": contentType, "Cache-Control": "no-store" });
    response.end(contents);
  } catch {
    denied(response);
  }
}

const server = http.createServer((request, response) => {
  const pathname = new URL(request.url ?? "/", "http://localhost").pathname;
  if (pathname.startsWith("/admin/")) {
    void managementRequest(request, response, pathname);
    return;
  }
  void staticAsset(response, pathname);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`p10-04 browser fixture listening on http://127.0.0.1:${port}\n`);
});
