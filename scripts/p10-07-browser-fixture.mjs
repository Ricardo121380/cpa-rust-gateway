#!/usr/bin/env node

// Local deterministic browser fixture for P10-07 Config Version lifecycle evidence. It serves
// only built static assets and synthetic non-secret lifecycle metadata. It has no Provider
// transport, persistent database, credential source, backup material, proxy, or external egress.

import http from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = path.join(root, "web/admin-ui/dist");
const port = Number.parseInt(process.env.P10_BROWSER_PORT ?? "4182", 10);
const fixtureSession = "configuration-browser-fixture";
const managementKey = ["mgmt", fixtureSession, "key"].join("_");
const csrfToken = ["csrf", fixtureSession].join("-");
const createdAtMs = 1_700_000_000_000;
const versions = new Map();
const auditEvents = [];
let activeVersionId;
let rollbackPredecessorId;

function responseJson(response, status, value) {
  response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
  response.end(JSON.stringify(value));
}

function denied(response) {
  responseJson(response, 404, { error: { code: "management_not_found", message: "Not found" } });
}

function conflict(response) {
  responseJson(response, 409, {
    error: {
      code: "management_lifecycle_conflict",
      message: "Management lifecycle operation is not available for the current configuration",
    },
  });
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

function exactVersion(version) {
  return {
    id: version.id,
    ...(version.parent_id === null ? {} : { parent_id: version.parent_id }),
    status: version.status,
    revision: "rev-0",
    created_at_ms: createdAtMs,
    description: version.description,
  };
}

function appendAudit(action, configVersionId, replacedConfigVersionId) {
  auditEvents.push({
    id: auditEvents.length + 1,
    action,
    actor: "management-key",
    occurred_at_ms: createdAtMs + auditEvents.length,
    config_version_id: configVersionId,
    ...(replacedConfigVersionId === undefined ? {} : { replaced_config_version_id: replacedConfigVersionId }),
  });
}

function authorized(request) {
  return request.headers["x-management-key"] === managementKey
    && (request.method === "GET" || request.headers["x-management-csrf-token"] === csrfToken);
}

async function lifecycleRequest(request, response, pathname) {
  if (!authorized(request)) {
    denied(response);
    return;
  }
  if (pathname === "/admin/config-versions" && request.method === "GET") {
    responseJson(response, 200, [...versions.values()].sort((left, right) => left.id.localeCompare(right.id)).map(exactVersion));
    return;
  }
  if (pathname === "/admin/audit-events" && request.method === "GET") {
    responseJson(response, 200, auditEvents);
    return;
  }
  if (pathname === "/admin/config-versions" && request.method === "POST") {
    const body = await readJson(request, response);
    if (body === undefined) return;
    if (!hasExactKeys(body, ["id", "parent_id", "description"])
      || typeof body.id !== "string" || body.id.length === 0 || body.id.length > 128
      || !(body.parent_id === null || typeof body.parent_id === "string")
      || typeof body.description !== "string" || body.description.trim().length === 0 || body.description.length > 1024
      || versions.has(body.id)) {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return;
    }
    const version = { id: body.id, parent_id: body.parent_id, status: "draft", description: body.description };
    versions.set(version.id, version);
    appendAudit("config_created", version.id);
    responseJson(response, 201, exactVersion(version));
    return;
  }
  if (pathname === "/admin/config-versions/rollback" && request.method === "POST") {
    if (request.headers["if-match"] !== "rev-0" || activeVersionId === undefined || rollbackPredecessorId === undefined) {
      conflict(response);
      return;
    }
    const restored = versions.get(rollbackPredecessorId);
    const replaced = versions.get(activeVersionId);
    if (restored === undefined || replaced === undefined || restored.status !== "archived") {
      conflict(response);
      return;
    }
    restored.status = "active";
    replaced.status = "archived";
    activeVersionId = restored.id;
    rollbackPredecessorId = replaced.id;
    appendAudit("config_rolled_back", restored.id, replaced.id);
    responseJson(response, 200, { active_config_version_id: restored.id, replaced_config_version_id: replaced.id });
    return;
  }
  const segments = pathname.split("/");
  if (segments.length < 4
    || segments[1] !== "admin"
    || segments[2] !== "config-versions"
    || segments[3].length === 0
    || segments.length > 5) {
    denied(response);
    return;
  }
  const versionId = segments[3];
  const action = segments[4];
  const version = versions.get(decodeURIComponent(versionId));
  if (version === undefined) {
    conflict(response);
    return;
  }
  if (action === undefined && request.method === "GET") {
    responseJson(response, 200, exactVersion(version));
    return;
  }
  if (action === "validate" && request.method === "POST") {
    responseJson(response, 200, { valid: true, error_codes: [] });
    return;
  }
  if (action === "publish" && request.method === "POST") {
    if (request.headers["if-match"] !== "rev-0" || version.status !== "draft") {
      conflict(response);
      return;
    }
    const replacedId = activeVersionId;
    if (replacedId !== undefined) versions.get(replacedId).status = "archived";
    version.status = "active";
    activeVersionId = version.id;
    rollbackPredecessorId = replacedId;
    appendAudit("config_published", version.id, replacedId);
    responseJson(response, 200, {
      active_config_version_id: version.id,
      ...(replacedId === undefined ? {} : { replaced_config_version_id: replacedId }),
    });
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
  const requestUrl = new URL(request.url ?? "/", "http://localhost");
  if (requestUrl.pathname === "/favicon.ico") {
    response.writeHead(204, { "Cache-Control": "no-store" });
    response.end();
    return;
  }
  if (requestUrl.pathname.startsWith("/admin/")) {
    void lifecycleRequest(request, response, requestUrl.pathname);
    return;
  }
  void staticAsset(response, requestUrl.pathname);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`p10-07 browser fixture listening on http://127.0.0.1:${port}\n`);
});
