// Typed management client. Header rules come from the contract (ops.gen.ts), not from call sites:
// X-Management-Key on every call, X-Management-CSRF-Token on unsafe methods, X-Config-Version where the
// operation declares it, and If-Match ("rev-N") where required. ETag responses refresh the working revision.
import createClient, { type Middleware } from "openapi-fetch";
import type { paths } from "./schema";
import { OPS } from "./ops.gen";
import { ApiError } from "./errors";
import { store } from "./session";

const UNSAFE = new Set(["POST", "PUT", "PATCH", "DELETE"]);

/**
 * Runtime projections exist only for the graph that is serving. Asked about a draft, the gateway answers
 * 503 (availability, catalog status) or 409 (egress status) — which would read as "not enabled" — so these
 * reads always name the active version, whatever version the operator is viewing.
 */
export const LIVE_READS = new Set(["/admin/runtime/availability", "/admin/catalog/status", "/admin/operations/provider-egress-status"]);

export function headersFor(method: string, schemaPath: string, explicit: Headers) {
  const { session, config } = store.get();
  const rule = OPS[`${method} ${schemaPath}`];
  const out: Record<string, string> = {};
  if (session.kind !== "none") out["X-Management-Key"] = session.token;
  if (UNSAFE.has(method) && session.kind !== "none" && session.csrf) out["X-Management-CSRF-Token"] = session.csrf;
  let target = explicit.get("X-Config-Version");
  if (rule?.cv && !target) {
    target = method === "GET" && LIVE_READS.has(schemaPath) ? config.active : config.working ?? config.active;
    if (target) out["X-Config-Version"] = target;
  }
  // The tracked revision belongs to the working version; never send it for a write aimed elsewhere.
  if (rule?.ifMatch && !explicit.has("If-Match") && config.revision && (!rule.cv || target === config.working)) out["If-Match"] = `"${config.revision.replace(/"/g, "")}"`;
  return out;
}

const auth: Middleware = {
  onRequest({ request, schemaPath }) {
    const add = headersFor(request.method, schemaPath, request.headers);
    for (const [k, v] of Object.entries(add)) request.headers.set(k, v);
    return request;
  },
  async onResponse({ request, response, schemaPath }) {
    const etag = response.headers.get("ETag");
    const { config } = store.get();
    // Graph reads and writes on the working version return its current revision as ETag ("rev-N").
    if (etag && response.ok && config.working && request.headers.get("X-Config-Version") === config.working) {
      const rev = etag.replace(/"/g, "");
      if (rev !== config.revision) store.setConfig({ revision: rev });
    }
    if (response.ok) return response;
    let code = "http_" + response.status, message = response.statusText;
    try { const body = await response.clone().json(); code = body?.error?.code ?? code; message = body?.error?.message ?? message; } catch { /* non-JSON error body */ }
    const retry = Number(response.headers.get("Retry-After")) || undefined;
    if (code === "management_access_denied" && !schemaPath.startsWith("/admin/auth/")) store.signOut();
    throw new ApiError(response.status, code, message, retry);
  },
};

let transport: typeof fetch = (...a) => globalThis.fetch(...a);
/** Swapped for the in-memory implementation in mock mode (VITE_CONSOLE_MOCK=1). */
export function useTransport(f: typeof fetch) { transport = f; }

// Same-origin by construction (the console is served from the management listener); explicit for non-browser runs.
export const api = createClient<paths>({ baseUrl: globalThis.location?.origin ?? "", fetch: (input) => transport(input) });
api.use(auth);

/** Unwraps openapi-fetch results so React Query sees thrown ApiErrors and typed data. */
export async function unwrap<T>(p: Promise<{ data?: T; error?: unknown; response: Response }>): Promise<T> {
  const r = await p;
  if (r.error !== undefined) throw r.error instanceof ApiError ? r.error : new ApiError(r.response.status, "http_" + r.response.status, String(r.error));
  return r.data as T;
}
