#!/usr/bin/env node

// Local deterministic browser-fixture server for P10-05 routing and Client Key evidence.
// It serves only built static assets and synthetic draft-management responses. It has no Provider
// transport, persistence, credential source, proxy, external egress, or usable Client Key.

import http from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = path.join(root, "web/admin-ui/dist");
const port = Number.parseInt(process.env.P10_BROWSER_PORT ?? "4180", 10);
const fixtureSession = "routing-browser-fixture";
const managementKey = ["mgmt", fixtureSession, "key"].join("_");
const csrfToken = ["csrf", fixtureSession].join("-");
let revision = 0;

function responseJson(response, status, value, nextRevision) {
  const headers = { "Content-Type": "application/json", "Cache-Control": "no-store" };
  if (nextRevision !== undefined) headers.ETag = `"rev-${nextRevision}"`;
  response.writeHead(status, headers);
  response.end(JSON.stringify(value));
}

function denied(response) {
  responseJson(response, 404, { error: { code: "management_not_found", message: "Not found" } });
}

async function readJson(request, response) {
  let body = "";
  for await (const chunk of request) {
    body += chunk;
    if (body.length > 70 * 1024) {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return undefined;
    }
  }
  try {
    return JSON.parse(body);
  } catch {
    responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
    return undefined;
  }
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasExactKeys(value, keys) {
  return isObject(value) && Object.keys(value).sort().join(",") === [...keys].sort().join(",");
}

async function managementRequest(request, response, pathname) {
  if (request.headers["x-management-key"] !== managementKey || request.headers["x-config-version"] !== "draft-p10") {
    denied(response);
    return;
  }
  if (request.method !== "GET" && request.headers["x-management-csrf-token"] !== csrfToken) {
    denied(response);
    return;
  }
  const mutates = ["POST", "PATCH", "DELETE"].includes(request.method);
  if (mutates && request.headers["if-match"] !== `rev-${revision}`) {
    responseJson(response, 409, { error: { code: "management_revision_conflict", message: "Management configuration changed" } });
    return;
  }
  if (request.method === "POST") {
    const body = await readJson(request, response);
    if (body === undefined) return;
    const next = () => ++revision;
    if (pathname === "/admin/public-models" && hasExactKeys(body, ["id", "model_name", "status", "display_name", "capabilities"])) {
      responseJson(response, 201, body, next());
      return;
    }
    if (pathname === "/admin/public-models/model-minimax-m3/aliases" && hasExactKeys(body, ["alias"])) {
      responseJson(response, 201, { ...body, public_model_id: "model-minimax-m3" }, next());
      return;
    }
    if (pathname === "/admin/public-models/model-minimax-m3/routes" && hasExactKeys(body, ["id", "policy", "max_attempts", "bootstrap_timeout_ms"])) {
      responseJson(response, 201, { ...body, public_model_id: "model-minimax-m3" }, next());
      return;
    }
    if (pathname === "/admin/routes/route-minimax-m3/candidates" && hasExactKeys(body, ["id", "endpoint_id", "upstream_model", "credential_scope", "transform_mode", "enabled", "priority", "weight", "capability_override"])) {
      responseJson(response, 201, { ...body, route_id: "route-minimax-m3" }, next());
      return;
    }
    if (pathname === "/admin/access-groups" && hasExactKeys(body, ["id", "name", "status", "limits"])) {
      responseJson(response, 201, body, next());
      return;
    }
    if (pathname === "/admin/access-groups/group-minimax-m3/routes" && hasExactKeys(body, ["route_id", "enabled"])) {
      responseJson(response, 201, { ...body, access_group_id: "group-minimax-m3" }, next());
      return;
    }
    if (pathname === "/admin/client-keys" && hasExactKeys(body, ["id", "access_group_id", "status", "expires_at_ms"])) {
      responseJson(response, 201, { ...body, prefix: "rgw_fixture", key: "rgw_fixture_display_only" }, next());
      return;
    }
  }
  if (pathname === "/admin/client-keys/client-minimax-m3" && request.method === "GET") {
    responseJson(response, 200, { id: "client-minimax-m3", access_group_id: "group-minimax-m3", prefix: "rgw_fixture", status: "active", expires_at_ms: null }, revision);
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
    response.writeHead(200, {
      "Content-Type": contentType,
      "Cache-Control": "no-store",
      "Content-Security-Policy": "default-src 'self'; base-uri 'none'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; object-src 'none'; script-src 'self'; style-src 'self'",
      "Referrer-Policy": "no-referrer",
    });
    response.end(contents);
  } catch {
    denied(response);
  }
}

const server = http.createServer((request, response) => {
  const pathname = new URL(request.url ?? "/", "http://localhost").pathname;
  if (pathname === "/favicon.ico") {
    response.writeHead(204, { "Cache-Control": "no-store" });
    response.end();
    return;
  }
  if (pathname.startsWith("/admin/")) {
    void managementRequest(request, response, pathname);
    return;
  }
  void staticAsset(response, pathname);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`p10-05 browser fixture listening on http://127.0.0.1:${port}\n`);
});
