// In-memory management API for VITE_CONSOLE_MOCK=1. It honours the same header rules as the gateway
// (X-Management-Key, CSRF on unsafe methods, If-Match revision, draft-only writes) so the console's
// conflict and draft flows can be exercised without a backend.
import * as D from "./seed";

type Ctx = { url: URL; method: string; headers: Headers; body: any; m: RegExpMatchArray };
type Handler = (c: Ctx) => unknown | Response;
const routes: [string, RegExp, Handler][] = [];
const on = (method: string, path: string, h: Handler) => routes.push([method, new RegExp("^" + path.replace(/\{[^}]+\}/g, "([^/]+)") + "$"), h]);

const json = (body: unknown, status = 200, headers: Record<string, string> = {}) => new Response(body === undefined ? null : JSON.stringify(body), { status, headers: { "Content-Type": "application/json", ...headers } });
const err = (status: number, code: string, message = code) => json({ error: { code, message } }, status);
const page = <T,>(items: T[], extra: Record<string, unknown> = {}) => ({ ...extra, items, next_cursor: null });

/* draft revisions: version id → numeric revision */
const rev: Record<string, number> = Object.fromEntries(D.versions.map((v) => [v.id, Number(v.revision.replace("rev-", ""))]));
const etag = (v: string) => ({ ETag: `"rev-${rev[v]}"` });
let session: { token: string; csrf: string; mustChange: boolean } | null = null;

function graphWrite(c: Ctx, apply: () => unknown, status = 200): Response {
  const v = c.headers.get("X-Config-Version");
  const ver = D.versions.find((x) => x.id === v);
  if (!ver) return err(400, "invalid_management_request", "X-Config-Version 缺失");
  if (ver.status !== "draft") return err(409, "management_version_not_writable", "只能修改草稿");
  const im = (c.headers.get("If-Match") || "").replace(/"/g, "");
  if (im !== `rev-${rev[ver.id]}`) return err(409, "management_revision_conflict", "revision mismatch");
  const out = apply();
  rev[ver.id] += 1; ver.revision = `rev-${rev[ver.id]}`;
  return json(out, status, etag(ver.id));
}
const vOf = (c: Ctx) => c.headers.get("X-Config-Version") || D.versions.find((v) => v.status === "active")!.id;

/* ---------------- auth ---------------- */
on("POST", "/admin/auth/login", (c) => {
  if (c.body?.username !== "admin" || !c.body?.password) return err(401, "management_login_invalid_credentials");
  session = { token: "session_" + "a".repeat(64), csrf: "csrf_" + "b".repeat(64), mustChange: c.body.password === "change-me-now" };
  return { username: "admin", session_token: session.token, csrf_token: session.csrf, expires_at_ms: Date.now() + 8 * 36e5, password_change_required: session.mustChange };
});
on("POST", "/admin/auth/password", (c) => { if ((c.body?.new_password ?? "").length < 12) return err(400, "management_login_invalid_password"); if (session) session.mustChange = false; return new Response(null, { status: 204 }); });
on("POST", "/admin/auth/logout", () => { session = null; return new Response(null, { status: 204 }); });

/* ---------------- system / versions / audit ---------------- */
on("GET", "/admin/system", () => ({ build: { version: "0.14.0-dev", build_revision: "c39bc07b", build_target: "aarch64-apple-darwin", rust_version: "1.95.0", schema_version: 42 }, uptime_seconds: 3 * 86400 + 5 * 3600 + 720, configuration_application: "live", accepting_requests: true, server_instance: "cpar-local" }));
on("GET", "/admin/config-versions", () => D.versions);
on("GET", "/admin/config-versions/{id}", (c) => D.versions.find((v) => v.id === c.m[1]) ?? err(404, "management_resource_not_found"));
on("POST", "/admin/config-versions/{id}/fork", (c) => {
  const from = D.versions.find((v) => v.id === c.m[1]); if (!from) return err(404, "management_resource_not_found");
  const v = { id: c.body.id, parent_id: from.id, status: "draft" as const, revision: "rev-0", created_at_ms: Date.now(), description: c.body.description || "" };
  rev[v.id] = 0; D.versions.unshift(v); return json(v, 201, etag(v.id));
});
on("POST", "/admin/config-versions/{id}/validate", (c) => (c.m[1] === "cv-0042" ? { valid: false, error_codes: ["missing_active_credential_binding"] } : { valid: true, error_codes: [] }));
on("POST", "/admin/config-versions/{id}/publish", (c) => {
  const v = D.versions.find((x) => x.id === c.m[1]); if (!v) return err(404, "management_resource_not_found");
  const old = D.versions.find((x) => x.status === "active")!; old.status = "archived"; v.status = "active";
  D.audit.unshift({ id: D.audit[0].id + 1, action: "config_version.publish", actor: "admin", occurred_at_ms: Date.now(), config_version_id: v.id, replaced_config_version_id: old.id });
  return { active_config_version_id: v.id, replaced_config_version_id: old.id };
});
on("POST", "/admin/config-versions/rollback", () => {
  const cur = D.versions.find((x) => x.status === "active")!; const v = D.versions.find((x) => x.id === cur.parent_id); if (!v) return err(409, "management_revision_conflict", "no previous version");
  const old = D.versions.find((x) => x.status === "active")!; old.status = "archived"; v.status = "active";
  return { active_config_version_id: v.id, replaced_config_version_id: old.id };
});
on("GET", "/admin/config-versions/{id}/diff", (c) => ({ base: { id: c.url.searchParams.get("base_id"), revision: "rev-45" }, target: { id: c.m[1], revision: `rev-${rev[c.m[1]] ?? 0}` }, items: c.m[1].startsWith("draft") ? draftChanges : [{ resource_kind: "endpoint", resource_key: "ep-mm-msg", change: "changed", changed_fields: ["adapter_id"] }, { resource_kind: "route_candidate", resource_key: "cd-m3-msg", change: "changed", changed_fields: ["transform_mode"] }], next_cursor: null }));
const draftChanges: { resource_kind: string; resource_key: string; change: string; changed_fields: string[] }[] = [];
on("GET", "/admin/audit-events", () => D.audit);
on("GET", "/admin/resource-audit-events", () => ({ items: D.resourceAudit, next_before_id: null }));

/* ---------------- graph reads ---------------- */
on("GET", "/admin/upstreams", (c) => json(D.upstreams, 200, etag(vOf(c))));
on("GET", "/admin/endpoints", (c) => page(D.endpoints, { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}`, observation_version: "obs-1" }));
on("GET", "/admin/endpoints/{id}/credential-bindings", (c) => D.bindings.filter((b) => b.endpoint_id === c.m[1]));
on("GET", "/admin/egress-policies", () => D.egress);
on("GET", "/admin/public-models", () => D.publicModels);
on("GET", "/admin/routes", (c) => page(D.routes, { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}` }));
on("GET", "/admin/route-candidates", (c) => page(D.candidates, { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}` }));
on("GET", "/admin/model-aliases", (c) => page(D.aliases, { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}` }));
on("GET", "/admin/access-groups", () => D.accessGroups);
on("GET", "/admin/access-groups/{id}/routes", (c) => D.grants.filter((g) => g.access_group_id === c.m[1]));
on("GET", "/admin/client-keys", () => D.clientKeys.map(({ _name, ...k }) => k));
on("GET", "/admin/compatible-proxy-pools", () => D.proxyPools);
on("GET", "/admin/compatible-proxy-nodes", () => D.proxyNodes);
on("GET", "/admin/compatible-egress-bindings", () => []);
on("GET", "/admin/routes/{id}/explain", (c) => {
  const cs = D.candidates.filter((x) => x.route_id === c.m[1]);
  return { route_id: c.m[1], price_policy: null, candidates: cs.map((x) => { const accs = D.accounts.filter((a) => a.ep === x.endpoint_id && a.status !== "disabled"); const ok = accs.some((a) => a.avail === "available"); const top = Math.min(...cs.map((y) => y.priority)); return { candidate_id: x.id, decision: ok && x.priority === top ? "selected" : "excluded", reason: !ok ? `no_available_credential: ${accs.map((a) => `${a.id} ${a.avail}`).join("; ")}` : x.priority === top ? `priority tier ${x.priority} · ${accs.filter((a) => a.avail === "available").length}/${accs.length} credentials available` : "lower_priority_tier", price_evidence: "not_evaluated" }; }) };
});

/* ---------------- accounts ---------------- */
const unified = (a: (typeof D.accounts)[number]) => {
  const managed = a.native ? null : { credential: D.credential(a), binding_count: 1, identity: D.identity(a), category: a.cat, provider: D.upstreams.find((u) => u.id === a.up)!.name, connections: [{ id: a.ep, api_format: D.endpoints.find((e) => e.id === a.ep)!.api_format, enabled: a.status !== "disabled", host: new URL(D.endpoints.find((e) => e.id === a.ep)!.base_url).host }], plan: a.plan, plan_source: a.plan ? "provider" : null, authentication: a.cat === "api" ? "api_key" : "oauth" };
  return { id: a.id, native: !!a.native, identity: D.identity(a), name: a.email ?? a.id, category: a.cat, provider: D.upstreams.find((u) => u.id === a.up)!.name, status: a.status === "disabled" ? "disabled" : a.status === "unauthorized" ? "reauth_required" : "enabled", operations: ["details", "enable", "disable", "reauthorize", "models"], managed, native_account: a.native ? { id: a.id, provider: a.native, auth_status: "active", enabled: true, revision: 4, import_batch_id: "batch-7", identity: D.identity(a), adapter_models: null } : null, plan: a.plan, plan_source: a.plan ? "provider" : null };
};
on("GET", "/admin/accounts/inventory", (c) => {
  const cat = c.url.searchParams.get("category"); const st = c.url.searchParams.get("status"); const q = (c.url.searchParams.get("q") || "").toLowerCase();
  const all = D.accounts.map(unified);
  const items = all.filter((x) => (!cat || x.category === cat) && (!st || x.status === st) && (!q || x.id.includes(q) || x.name.toLowerCase().includes(q)));
  const totals: Record<string, number> = {}; all.forEach((x) => (totals[x.category] = (totals[x.category] ?? 0) + 1));
  const plans: Record<string, number> = {}; all.forEach((x) => x.plan && (plans[x.plan] = (plans[x.plan] ?? 0) + 1));
  return { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}`, total: all.length, category_totals: totals, items, next_cursor: null, plan_totals: plans, unobserved_plan_total: all.filter((x) => !x.plan).length };
});
on("GET", "/admin/credentials", (c) => page(D.accounts.filter((a) => !a.native).map((a) => unified(a).managed), { config_version: vOf(c), revision: `rev-${rev[vOf(c)]}`, observation_version: "obs-1" }));
const rtStatus: Record<string, string> = { available: "available", cooldown: "cooling", quota_blocked: "quota_blocked", circuit_open: "circuit_open", credential_unauthorized: "unauthorized", recovery_required: "recovery_in_flight", credential_forbidden: "unauthorized", expired: "expired" };
on("GET", "/admin/operations/provider-account-pools", () => ({ snapshot_id: "snap-" + Math.floor(Date.now() / 60000), observed_at_ms: Date.now(), next_cursor: null, items: D.accounts.map((a) => ({ credential_revision: 4, config_version_id: "cv-0045", provider_id: a.up, channel_id: a.ep, account_id: a.id, account_kind: a.cat, auth_status: a.status === "unauthorized" ? "reauth_required" : a.status === "disabled" ? "disabled" : "active", runtime_status: rtStatus[a.avail], enabled: a.status !== "disabled", priority: a.pr, weight: a.w, max_concurrency: a.conc, active_leases: a.status === "active" ? Math.floor(D.R() * Math.min(a.conc, 5)) : 0, expires_at_ms: a.cat === "api" ? null : Date.now() + 9 * 864e5, refresh_due_at_ms: null, quota_sync_due_at_ms: D.quota[a.id] ? Date.now() + 5 * 6e4 : null, entitlement: a.plan ? { domain: a.cat === "grok" ? "grok_build" : a.cat === "claude" ? "claude" : "chatgpt", tier: a.plan, source: "provider_subscription", confidence: "authoritative", observed_at_ms: Date.now() - 36e5 } : null, presentation: { identity: D.identity(a), category: a.cat, provider: D.upstreams.find((u) => u.id === a.up)!.name, api_format: D.endpoints.find((e) => e.id === a.ep)!.api_format, host: null, source: null } })) }));
on("POST", "/admin/operations/provider-account-pools/actions", (c) => ({ state: c.body?.action === "cool_down" ? "cooling" : "probe_scheduled", observed_at_ms: Date.now(), audit_recorded: true, cooldown_until_ms: c.body?.action === "cool_down" ? Date.now() + (c.body.cooldown_ms ?? 6e5) : null }));
on("GET", "/admin/operations/provider-account-pools/failures", () => ({ observed_through_ordinal: 9123, items: D.failures, next_cursor: null }));
on("GET", "/admin/credentials/{id}/metadata", (c) => { const a = D.A[c.m[1]]; return a ? { credential_id: a.id, kind: D.credential(a).kind, revision: 4, plan: a.plan, email: a.email, platform: a.cat, source_format: "direct_oauth", live_quota: D.quota[a.id] ?? null, live_quota_error: D.quota[a.id] ? null : a.cat === "api" ? "not_implemented_or_not_connected" : "unauthorized" } : err(404, "management_resource_not_found"); });
on("GET", "/admin/native-accounts/{id}/usage", (c) => ({ observation: D.quota[c.m[1]] ?? null, error: D.quota[c.m[1]] ? null : "not_connected" }));
on("POST", "/admin/upstreams/{id}/codex-authorization/start", (c) => ({ credential_id: c.body.id, state: "pending", expires_at_ms: Date.now() + 6e5, authorization_url: "https://auth.openai.com/oauth/authorize?response_type=code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code_challenge_method=S256&state=st_8f2c", session_id: "sess-codex-1" }));
on("POST", "/admin/upstreams/{id}/claude-authorization/start", (c) => ({ credential_id: c.body.id, state: "pending", expires_at_ms: Date.now() + 6e5, authorization_url: "https://claude.ai/oauth/authorize?response_type=code&code_challenge_method=S256&state=st_31aa", session_id: "sess-claude-1" }));
on("POST", "/admin/upstreams/{id}/codex-authorization/callback", (c) => ({ ...D.credential(D.accounts[0]), id: c.body?.id ?? "cr-codex-new" }));
on("POST", "/admin/upstreams/{id}/claude-authorization/callback", (c) => ({ ...D.credential(D.accounts[4]), id: c.body?.id ?? "cr-claude-new" }));
let polls = 0;
on("POST", "/admin/upstreams/{id}/kiro-authorization/start", () => { polls = 0; return { state: "pending", session_id: "sess-kiro-1", user_code: "QZTK-WMRX", verification_uri: "https://view.awsapps.com/start/#/device", expires_at_ms: Date.now() + 6e5, interval_ms: 5000 }; });
on("POST", "/admin/upstreams/{id}/kiro-authorization/poll", () => (++polls >= 2 ? { state: "completed", credential_id: "cr-kiro-02" } : { state: "pending", session_id: "sess-kiro-1", user_code: "QZTK-WMRX", verification_uri: "https://view.awsapps.com/start/#/device", interval_ms: 5000 }));
on("POST", "/admin/upstreams/{id}/kimi-authorization/start", () => { polls = 0; return { state: "pending", session_id: "sess-kimi-1", user_code: "KM-4821", verification_uri: "https://www.kimi.com/device", expires_at_ms: Date.now() + 6e5, interval_ms: 5000 }; });
on("POST", "/admin/upstreams/{id}/kimi-authorization/poll", () => (++polls >= 2 ? { state: "completed", credential_id: "cr-kimi-02" } : { state: "pending", session_id: "sess-kimi-1", user_code: "KM-4821", verification_uri: "https://www.kimi.com/device", interval_ms: 5000 }));
on("POST", "/admin/upstreams/{id}/kiro-authorization/cancel", () => ({ state: "cancelled" }));
on("POST", "/admin/upstreams/{id}/kimi-authorization/cancel", () => ({ state: "cancelled" }));
on("POST", "/admin/native-account-authorizations", () => { polls = 0; return { session_id: "sess-grok-1", state: "pending", user_code: "GROK-77Q2", verification_uri: "https://accounts.x.ai/device", expires_at_ms: Date.now() + 6e5, retry_at_ms: Date.now() + 5000, identity: null, identity_state: "pending" }; });
on("POST", "/admin/native-account-authorizations/{id}/poll", () => (++polls >= 2 ? { session_id: "sess-grok-1", state: "complete", user_code: "GROK-77Q2", verification_uri: "https://accounts.x.ai/device", expires_at_ms: Date.now() + 6e5, retry_at_ms: Date.now(), identity: { email: "new***@x.com", phone: null, username: null }, identity_state: "observed" } : { session_id: "sess-grok-1", state: "pending", user_code: "GROK-77Q2", verification_uri: "https://accounts.x.ai/device", expires_at_ms: Date.now() + 6e5, retry_at_ms: Date.now() + 5000, identity: null, identity_state: "pending" }));
on("DELETE", "/admin/native-account-authorizations/{id}", () => ({ state: "cancelled" }));
on("POST", "/admin/upstreams/{id}/account-import", (c) => graphWrite(c, () => { draftChanges.push({ resource_kind: "credential", resource_key: c.body.id, change: "added", changed_fields: [] }); return { id: c.body.id, upstream_id: c.m[1], kind: "api_key", status: "active", revision: 1, secret_present: true }; }, 201));

/* ---------------- graph writes (draft only) ---------------- */
on("PATCH", "/admin/credentials/{id}/status", (c) => graphWrite(c, () => { const a = D.A[c.m[1]]; if (a) a.status = c.body.status; draftChanges.push({ resource_kind: "credential", resource_key: c.m[1], change: "changed", changed_fields: ["status"] }); return a ? D.credential(a) : undefined; }));
on("PATCH", "/admin/native-accounts/{id}", (c) => { const a = D.A[c.m[1]]; if (a) a.status = c.body.enabled ? "active" : "disabled"; return { account_id: c.m[1], revision: 5, enabled: c.body.enabled }; });
const patchList = (list: any[], key: string) => (c: Ctx) => graphWrite(c, () => { const x = list.find((y) => y[key] === c.m[1]); Object.assign(x, c.body); draftChanges.push({ resource_kind: key === "id" ? "resource" : key, resource_key: c.m[1], change: "changed", changed_fields: Object.keys(c.body ?? {}) }); return x; });
on("PATCH", "/admin/upstreams/{id}", patchList(D.upstreams, "id"));
on("PATCH", "/admin/client-keys/{id}", patchList(D.clientKeys, "id"));
on("PATCH", "/admin/access-groups/{id}", patchList(D.accessGroups, "id"));
on("PATCH", "/admin/public-models/{id}", patchList(D.publicModels, "id"));
on("PATCH", "/admin/compatible-proxy-nodes/{id}", patchList(D.proxyNodes, "id"));
on("PATCH", "/admin/routes/{rid}/candidates/{id}", (c) => graphWrite(c, () => { const x = D.candidates.find((y) => y.id === c.m[2]); if (x) Object.assign(x, c.body); draftChanges.push({ resource_kind: "route_candidate", resource_key: c.m[2], change: "changed", changed_fields: ["enabled"] }); return x; }));
on("POST", "/admin/endpoints/{id}/credential-bindings", (c) => graphWrite(c, () => { const b = D.bindings.find((x) => x.endpoint_id === c.m[1] && x.credential_id === c.body.credential_id); if (b) Object.assign(b, c.body); draftChanges.push({ resource_kind: "binding", resource_key: `${c.m[1]}/${c.body.credential_id}`, change: "changed", changed_fields: ["enabled"] }); return b; }, 201));
on("POST", "/admin/access-groups/{id}/routes", (c) => graphWrite(c, () => { const g = D.grants.find((x) => x.access_group_id === c.m[1] && x.route_id === c.body.route_id); if (g) g.enabled = c.body.enabled; else D.grants.push({ access_group_id: c.m[1], route_id: c.body.route_id, enabled: c.body.enabled }); draftChanges.push({ resource_kind: "access_group_route", resource_key: `${c.m[1]}/${c.body.route_id}`, change: g ? "changed" : "added", changed_fields: ["enabled"] }); return { access_group_id: c.m[1], ...c.body }; }, 201));
on("POST", "/admin/client-keys", (c) => graphWrite(c, () => { const k = { id: c.body.id, access_group_id: c.body.access_group_id, prefix: "rgw_Q8mT", status: "active" as const, expires_at_ms: c.body.expires_at_ms ?? null, last_request_at_ms: null, _name: c.body.id }; D.clientKeys.unshift(k); draftChanges.push({ resource_kind: "client_key", resource_key: k.id, change: "added", changed_fields: [] }); return { ...k, key: "rgw_Q8mT2vR7xK4nP9wL6cJ3hB5fD1sA0eYuW2" }; }, 201));
on("DELETE", "/admin/client-keys/{id}", (c) => graphWrite(c, () => { const k = D.clientKeys.find((x) => x.id === c.m[1]); if (k) k.status = "revoked"; draftChanges.push({ resource_kind: "client_key", resource_key: c.m[1], change: "changed", changed_fields: ["status"] }); return undefined; }));
on("POST", "/admin/endpoints/{id}/test", (c) => (c.m[1] === "ep-kimi" ? { outcome: "rejected", status_class: "4xx", canonical_lifecycle: false } : { outcome: "pass", status_class: "2xx", canonical_lifecycle: true }));
on("POST", "/admin/endpoints/{id}/models/discover-preview", () => ({ added: 3, removed: 1, unchanged: 14 }));
on("POST", "/admin/endpoints/{id}/models/discover-apply", (c) => graphWrite(c, () => ({ added: 3, removed: 1, unchanged: 14 })));

/* ---------------- runtime ---------------- */
on("GET", "/admin/runtime/availability", () => D.accounts.filter((a) => a.status !== "disabled").map((a) => ({ endpoint_id: a.ep, credential_id: a.id, availability: a.avail })));
on("POST", "/admin/runtime/quota/reset", () => ({ state: "probe_scheduled" }));
on("GET", "/admin/catalog/status", () => D.catalogStatus);
on("GET", "/admin/observability/metrics", () => new Response(D.METRICS, { headers: { "Content-Type": "text/plain; version=0.0.4" } }));
on("GET", "/admin/operations/provider-egress-status", () => err(503, "management_runtime_unavailable", "provider egress status is not configured"));
on("GET", "/admin/operations/billing-processing", () => ({ state: "current", observed_at_ms: Date.now(), source_ordinal: 9123, checkpoint_ordinal: 9123, checkpoint_updated_at_ms: Date.now() - 40000, unresolved_failures: 0, failure_code: null, quarantined_failures: 0 }));

/* ---------------- observation ---------------- */
function filterReq(c: Ctx) {
  const p = c.url.searchParams; const from = Number(p.get("from_ms") || 0), to = Number(p.get("to_ms") || Date.now() + 1);
  return D.requests.filter((r) => r.started_at_ms! >= from && r.started_at_ms! < to && (!p.get("model") || r.model === p.get("model")) && (!p.get("outcome") || r.outcome === p.get("outcome")) && (!p.get("upstream_id") || r.upstream_id === p.get("upstream_id")) && (!p.get("credential_id") || r.credential_id === p.get("credential_id")) && (!p.get("client_key_id") || r.client_key_id === p.get("client_key_id")) && (!p.get("request_id") || r.request_id === p.get("request_id")));
}
function summary(c: Ctx) {
  const p = c.url.searchParams; const from = Number(p.get("from_ms") || Date.now() - 7 * 864e5); const bucket = Number(p.get("bucket_ms") || 36e5);
  const hours = D.hourly.filter((h) => h.t >= from - 36e5);
  const reqs = hours.reduce((s, h) => s + h.req, 0), fails = hours.reduce((s, h) => s + h.fail, 0);
  const series: { at_ms: number; requests: number; succeeded: number; failed: number; cancelled: number; average_duration_ms: number; average_first_content_ms: number }[] = [];
  for (const h of hours) {
    const at = Math.floor(h.t / bucket) * bucket; const last = series[series.length - 1];
    if (bucket < 36e5) { const n = Math.round(36e5 / bucket); for (let k = 0; k < n; k++) { const share = (0.7 + D.R() * 0.6) / n; const r = Math.round(h.req * share); const f = k === 2 ? h.fail : 0; series.push({ at_ms: h.t + k * bucket, requests: r, succeeded: r - f, failed: f, cancelled: 0, average_duration_ms: h.dur, average_first_content_ms: h.first }); } continue; }
    if (last && last.at_ms === at) { last.requests += h.req; last.failed += h.fail; last.succeeded += h.req - h.fail; } else series.push({ at_ms: at, requests: h.req, succeeded: h.req - h.fail, failed: h.fail, cancelled: 0, average_duration_ms: h.dur, average_first_content_ms: h.first });
  }
  return { requests: reqs, succeeded: reqs - fails, failed: fails, cancelled: 4, unknown: 0, attempts: Math.round(reqs * 1.06), success_rate: (reqs - fails) / reqs, average_duration_ms: 2480, average_first_content_ms: 812, p50_duration_ms: 1960, p95_duration_ms: 8240, series: series.filter((s) => s.at_ms >= from - bucket) };
}
on("GET", "/admin/requests", (c) => { const items = filterReq(c); const lim = Number(c.url.searchParams.get("limit") || 50); const cur = Number(c.url.searchParams.get("cursor") || 0); const s = summary(c); return { snapshot: 9123, next_cursor: cur + lim < items.length ? String(cur + lim) : null, items: items.slice(cur, cur + lim), summary: { ...s, series: undefined }, series: s.series, ledger_snapshot: 9100 }; });
on("GET", "/admin/requests/summary", (c) => { const s = summary(c); const { series, ...sum } = s; return { snapshot: 9123, next_cursor: null, items: [], summary: sum, series, ledger_snapshot: 9100 }; });
on("GET", "/admin/requests/{id}/attempts", (c) => D.attempts[c.m[1]] ?? err(404, "management_resource_not_found"));
on("GET", "/admin/operations/usage", () => ({ observed_through_ms: Date.now() - 30000, items: D.usage, next_cursor: null, excluded_usage_events: 37, excluded_request_groups: 4 }));
on("GET", "/admin/operations/billing", (c) => {
  const st = c.url.searchParams.get("status");
  const items = D.requests.filter((r) => r.ledger_records > 0 && r.usage).slice(0, 80).map((r, i) => r.usage && ({ ledger_id: 9100 - i, request_id: r.request_id, response_id: "resp_" + r.request_id.slice(4), provider_id: r.upstream_id!, channel_id: r.endpoint_id!, account_id: r.credential_id!, model: r.model, input_tokens: r.usage.input_tokens, output_tokens: r.usage.output_tokens, reasoning_tokens: r.usage.reasoning_tokens, cache_read_tokens: r.usage.cache_read_tokens, cache_creation_tokens: r.usage.cache_creation_tokens, cached_tokens: r.usage.cached_tokens, usage_provenance: r.usage.provenance, input_accounting: r.usage.input_accounting, occurred_at_ms: r.finished_at_ms!, catalog_version_id: r.cost_confidence === "unpriced" ? null : "price-2026-10", cost_microunits: r.cost_microunits, cost_confidence: r.cost_confidence })).filter((x): x is NonNullable<typeof x> => !!x && (!st || x.cost_confidence === st));
  const n = (k: string) => items.filter((x) => x.cost_confidence === k).length;
  return { snapshot_ledger_id: 9100, items, summary: { records: items.length, exact_records: n("exact"), partial_records: n("partial"), unknown_records: n("unknown"), unpriced_records: n("unpriced"), known_cost_microunits: items.reduce((s, x) => s + (x.cost_microunits ?? 0), 0) }, next_cursor: null };
});
on("GET", "/admin/billing/catalogs", () => D.billingCatalogs);
on("GET", "/admin/billing/routing-price-policy", () => err(404, "management_resource_not_found", "no routing price policy"));
on("POST", "/admin/billing/price-source/refresh", () => ({ source: "models.dev", fetched_at_ms: Date.now(), models: 1342, matched: 6 }));

/* ---------------- backup ---------------- */
on("POST", "/admin/backups/preflight", () => ({ schema_version: 42, secret_key_required: true }));
on("POST", "/admin/restores/preflight", () => ({ schema_version: 42, quick_check_required: true, compatible: true }));

export async function mockFetch(input: Request): Promise<Response> {
  const url = new URL(input.url);
  await new Promise((r) => setTimeout(r, 120 + Math.random() * 220));
  const unsafe = !["GET", "HEAD"].includes(input.method);
  const key = input.headers.get("X-Management-Key");
  if (!url.pathname.startsWith("/admin/auth/login") && !key) return err(404, "management_access_denied");
  if (unsafe && key?.startsWith("session_") && input.headers.get("X-Management-CSRF-Token") !== session?.csrf && !url.pathname.startsWith("/admin/auth/login")) return err(404, "management_access_denied");
  for (const [method, re, h] of routes) {
    if (method !== input.method) continue;
    const m = url.pathname.match(re);
    if (!m) continue;
    let body: unknown = undefined;
    if (unsafe) { try { body = await input.clone().json(); } catch { /* empty body */ } }
    const out = h({ url, method: input.method, headers: input.headers, body, m });
    return out instanceof Response ? out : json(out);
  }
  return err(503, "management_runtime_unavailable", `mock has no handler for ${input.method} ${url.pathname}`);
}
