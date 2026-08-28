#!/usr/bin/env node

// Local deterministic browser fixture for P10-06 runtime-management evidence. It serves only
// built static assets and synthetic value-free management results. It has no Provider client,
// persistence, credential source, proxy, external egress, or recovery/probe capability.

import http from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = path.join(root, "web/admin-ui/dist");
const port = Number.parseInt(process.env.P10_BROWSER_PORT ?? "4181", 10);
const fixtureSession = "runtime-browser-fixture";
const managementKey = ["mgmt", fixtureSession, "key"].join("_");
const csrfToken = ["csrf", fixtureSession].join("-");

function responseJson(response, status, value) {
  response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
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

function hasExactKeys(value, keys) {
  return value !== null
    && typeof value === "object"
    && !Array.isArray(value)
    && Object.keys(value).sort().join(",") === [...keys].sort().join(",");
}

function authorized(request, pathname) {
  if (request.headers["x-management-key"] !== managementKey) return false;
  if (pathname !== "/admin/requests/request-runtime/attempts" && request.headers["x-config-version"] !== "draft-p10") return false;
  return request.method === "GET" || request.headers["x-management-csrf-token"] === csrfToken;
}

async function managementRequest(request, response, pathname, query) {
  if (!authorized(request, pathname)) {
    denied(response);
    return;
  }
  if (pathname === "/admin/catalog/status" && request.method === "GET") {
    responseJson(response, 200, [{ endpoint_id: "endpoint-runtime", credential_id: "credential-runtime", freshness: "fresh", observed_at_ms: 1_700_000_000_000 }]);
    return;
  }
  if (pathname === "/admin/runtime/availability" && request.method === "GET") {
    responseJson(response, 200, [{ endpoint_id: "endpoint-runtime", credential_id: "credential-runtime", availability: "recovery_required" }]);
    return;
  }
  if (pathname === "/admin/runtime/quota/reset" && request.method === "POST") {
    const body = await readJson(request, response);
    if (body === undefined) return;
    if (!hasExactKeys(body, ["endpoint_id", "credential_id", "upstream_model"])
      || body.endpoint_id !== "endpoint-runtime"
      || body.credential_id !== "credential-runtime"
      || body.upstream_model !== "runtime-model") {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return;
    }
    responseJson(response, 202, { state: "probe_scheduled" });
    return;
  }
  if (pathname === "/admin/routes/route-runtime/explain" && request.method === "GET") {
    if (query.get("requested_model") !== "public-runtime" || query.get("protocol") !== "openai_responses") {
      responseJson(response, 400, { error: { code: "invalid_management_request", message: "Management request is invalid" } });
      return;
    }
    responseJson(response, 200, {
      route_id: "route-runtime",
      candidates: [
        { candidate_id: "candidate-blocked", decision: "excluded", reason: "endpoint_cooldown" },
        { candidate_id: "candidate-selected", decision: "selected" },
      ],
    });
    return;
  }
  if (pathname === "/admin/requests/request-runtime/attempts" && request.method === "GET") {
    responseJson(response, 200, [{ attempt_id: "attempt-runtime-1", outcome: "succeeded", endpoint_id: "endpoint-runtime", credential_id: "credential-runtime" }]);
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
    void managementRequest(request, response, requestUrl.pathname, requestUrl.searchParams);
    return;
  }
  void staticAsset(response, requestUrl.pathname);
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`p10-06 browser fixture listening on http://127.0.0.1:${port}\n`);
});
