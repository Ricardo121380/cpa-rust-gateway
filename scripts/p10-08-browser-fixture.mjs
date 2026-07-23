#!/usr/bin/env node

// Local deterministic browser fixture for P10-08 recovery UI evidence. It serves only built
// static assets and synthetic value-free backup metadata. It has no database, Credential, Backup
// Key, Master Key, artifact persistence, provider transport, proxy, or external egress.

import http from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = path.join(root, "web/admin-ui/dist");
const port = Number.parseInt(process.env.P10_BROWSER_PORT ?? "4183", 10);
const fixtureSession = "backup-browser-fixture";
const managementKey = ["mgmt", fixtureSession, "key"].join("_");
const csrfToken = ["csrf", fixtureSession].join("-");
const maximumArtifactBytes = 16 * 1024 * 1024;

function responseJson(response, status, value) {
  response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
  response.end(JSON.stringify(value));
}

function denied(response) {
  responseJson(response, 404, { error: { code: "management_not_found", message: "Not found" } });
}

function authorized(request) {
  return request.headers["x-management-key"] === managementKey
    && request.headers["x-management-csrf-token"] === csrfToken;
}

async function readBoundedBinary(request, response) {
  const contentType = request.headers["content-type"]?.split(";", 1)[0]?.trim().toLowerCase();
  if (contentType !== "application/octet-stream") {
    responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
    return false;
  }
  let length = 0;
  for await (const chunk of request) {
    length += chunk.length;
    if (length > maximumArtifactBytes) {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return false;
    }
  }
  if (length === 0) {
    responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
    return false;
  }
  return true;
}

async function managementRequest(request, response, pathname) {
  if (!authorized(request)) {
    denied(response);
    return;
  }
  if (pathname === "/admin/backups/preflight" && request.method === "POST") {
    responseJson(response, 200, { schema_version: 9, secret_key_required: true });
    return;
  }
  if (pathname === "/admin/restores/preflight" && request.method === "POST") {
    if (await readBoundedBinary(request, response)) {
      responseJson(response, 200, { schema_version: 9, quick_check_required: true, compatible: true });
    }
    return;
  }
  if (pathname === "/admin/restores" && request.method === "POST") {
    if (await readBoundedBinary(request, response)) {
      responseJson(response, 202, { state: "complete" });
    }
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
    void managementRequest(request, response, requestUrl.pathname);
    return;
  }
  void staticAsset(response, requestUrl.pathname);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`p10-08 browser fixture listening on http://127.0.0.1:${port}\n`);
});
