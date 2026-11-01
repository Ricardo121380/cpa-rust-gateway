// Development-only seed data in exact contract shapes (docs/openapi/management-v1.json).
// Deterministic so screenshots and tests are stable. Never imported by production builds.
import type { S } from "../hooks";

function rng(seed: number) {
  return () => { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
export const R = rng(20261101);
const pick = <T,>(a: readonly T[]) => a[Math.floor(R() * a.length)];
export const NOW = Math.floor(Date.now() / 60000) * 60000;
const MIN = 6e4, HOUR = 36e5, DAY = 864e5;

/* ---------------- configuration graph ---------------- */
export const upstreams: S<"Upstream">[] = [
  { id: "up-codex", name: "Codex 官方", kind: "codex", enabled: true, tags: ["oauth", "official"], egress_policy_id: "eg-openai" },
  { id: "up-claude", name: "Claude 官方", kind: "claude", enabled: true, tags: ["oauth", "pkce"], egress_policy_id: "eg-anthropic" },
  { id: "up-grok", name: "Grok", kind: "grok.official", enabled: true, tags: ["native", "device-code"], egress_policy_id: "eg-xai" },
  { id: "up-minimax", name: "MiniMax", kind: "openai-compatible", enabled: true, tags: ["api-key"], egress_policy_id: "eg-minimax" },
  { id: "up-kiro", name: "Kiro", kind: "kiro", enabled: true, tags: ["device-code"], egress_policy_id: "eg-aws" },
  { id: "up-kimi", name: "Kimi", kind: "kimi-coding", enabled: false, tags: ["device-code"], egress_policy_id: "eg-moonshot" },
];
export const endpoints: S<"Endpoint">[] = [
  { id: "ep-codex", upstream_id: "up-codex", adapter_id: "openai-compatible.responses", api_format: "openai_responses", base_url: "https://chatgpt.com/backend-api/codex", inference_path: "/responses", models_path: null, transport: "https", enabled: true },
  { id: "ep-claude", upstream_id: "up-claude", adapter_id: "anthropic-compatible.messages", api_format: "anthropic_messages", base_url: "https://api.anthropic.com", inference_path: "/v1/messages", models_path: "/v1/models", transport: "https", enabled: true },
  { id: "ep-grok-build", upstream_id: "up-grok", adapter_id: "grok.build.responses", api_format: "openai_responses", base_url: "https://cli-chat-proxy.grok.com/v1", inference_path: "/responses", models_path: "/models", transport: "https", enabled: true },
  { id: "ep-mm-chat", upstream_id: "up-minimax", adapter_id: "openai-compatible.chat-completions", api_format: "openai_chat_completions", base_url: "https://api.minimax.io/v1", inference_path: "/chat/completions", models_path: "/models", transport: "https", enabled: true },
  { id: "ep-mm-msg", upstream_id: "up-minimax", adapter_id: "anthropic-compatible.messages", api_format: "anthropic_messages", base_url: "https://api.minimax.io/anthropic", inference_path: "/v1/messages", models_path: null, transport: "https", enabled: true },
  { id: "ep-kiro", upstream_id: "up-kiro", adapter_id: "kiro.messages", api_format: "anthropic_messages", base_url: "https://q.us-east-1.amazonaws.com", inference_path: "/generateAssistantResponse", models_path: "/ListAvailableModels", transport: "https", enabled: true },
  { id: "ep-kimi", upstream_id: "up-kimi", adapter_id: "openai-compatible.chat-completions", api_format: "openai_chat_completions", base_url: "https://api.kimi.com/coding/v1", inference_path: "/chat/completions", models_path: "/models", transport: "https", enabled: false },
];
export const egress: S<"EgressPolicy">[] = [
  { id: "eg-openai", name: "OpenAI / ChatGPT", allowed_schemes: ["https"], allowed_hosts: ["chatgpt.com", "auth.openai.com"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "deny", max_redirects: 0 },
  { id: "eg-anthropic", name: "Anthropic", allowed_schemes: ["https"], allowed_hosts: ["api.anthropic.com", "console.anthropic.com"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "deny", max_redirects: 0 },
  { id: "eg-xai", name: "xAI", allowed_schemes: ["https"], allowed_hosts: ["cli-chat-proxy.grok.com", "api.x.ai", "accounts.x.ai"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "revalidate", max_redirects: 2 },
  { id: "eg-minimax", name: "MiniMax", allowed_schemes: ["https"], allowed_hosts: ["api.minimax.io"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "deny", max_redirects: 0 },
  { id: "eg-aws", name: "AWS · Kiro", allowed_schemes: ["https"], allowed_hosts: ["q.us-east-1.amazonaws.com", "oidc.us-east-1.amazonaws.com"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "revalidate", max_redirects: 1 },
  { id: "eg-moonshot", name: "Moonshot · Kimi", allowed_schemes: ["https"], allowed_hosts: ["api.kimi.com", "auth.kimi.com"], allowed_ports: [443], allowed_cidrs: [], redirect_mode: "deny", max_redirects: 0 },
];

type Acc = { id: string; up: string; ep: string; cat: "codex" | "claude" | "api" | "kiro" | "kimi" | "grok"; email: string | null; plan: string | null; status: S<"Credential">["status"]; avail: S<"RuntimeAvailability">[number]["availability"]; pr: number; w: number; conc: number; share: number; fail: number; native?: "grok_build" | "grok_console" | "grok_web" };
export const accounts: Acc[] = [
  { id: "cr-codex-01", up: "up-codex", ep: "ep-codex", cat: "codex", email: "li***@outlook.com", plan: "pro", status: "active", avail: "available", pr: 0, w: 10, conc: 8, share: 0.13, fail: 0.004 },
  { id: "cr-codex-02", up: "up-codex", ep: "ep-codex", cat: "codex", email: "ops***@gmail.com", plan: "pro", status: "active", avail: "available", pr: 0, w: 10, conc: 8, share: 0.12, fail: 0.006 },
  { id: "cr-codex-03", up: "up-codex", ep: "ep-codex", cat: "codex", email: "dev***@cpar.dev", plan: "plus", status: "cooling", avail: "quota_blocked", pr: 1, w: 4, conc: 4, share: 0.04, fail: 0.031 },
  { id: "cr-codex-04", up: "up-codex", ep: "ep-codex", cat: "codex", email: "an***@icloud.com", plan: "plus", status: "unauthorized", avail: "credential_unauthorized", pr: 1, w: 4, conc: 4, share: 0.02, fail: 0.11 },
  { id: "cr-claude-01", up: "up-claude", ep: "ep-claude", cat: "claude", email: "cl***@proton.me", plan: "max20x", status: "active", avail: "available", pr: 0, w: 8, conc: 6, share: 0.12, fail: 0.005 },
  { id: "cr-claude-02", up: "up-claude", ep: "ep-claude", cat: "claude", email: "ka***@gmail.com", plan: "pro", status: "active", avail: "cooldown", pr: 0, w: 4, conc: 4, share: 0.05, fail: 0.018 },
  { id: "na-grok-01", up: "up-grok", ep: "ep-grok-build", cat: "grok", email: "bu***@x.com", plan: "heavy", status: "active", avail: "available", pr: 0, w: 8, conc: 12, share: 0.13, fail: 0.004, native: "grok_build" },
  { id: "na-grok-02", up: "up-grok", ep: "ep-grok-build", cat: "grok", email: "gr***@proton.me", plan: "supergrok", status: "active", avail: "available", pr: 0, w: 6, conc: 8, share: 0.08, fail: 0.006, native: "grok_build" },
  { id: "na-grok-03", up: "up-grok", ep: "ep-grok-build", cat: "grok", email: "we***@x.com", plan: "free", status: "active", avail: "circuit_open", pr: 2, w: 1, conc: 2, share: 0.02, fail: 0.045, native: "grok_web" },
  { id: "cr-mm-01", up: "up-minimax", ep: "ep-mm-chat", cat: "api", email: null, plan: null, status: "active", avail: "available", pr: 0, w: 6, conc: 16, share: 0.1, fail: 0.003 },
  { id: "cr-mm-02", up: "up-minimax", ep: "ep-mm-msg", cat: "api", email: null, plan: null, status: "active", avail: "available", pr: 0, w: 4, conc: 16, share: 0.06, fail: 0.004 },
  { id: "cr-kiro-01", up: "up-kiro", ep: "ep-kiro", cat: "kiro", email: "ki***@builder.id", plan: "pro", status: "active", avail: "recovery_required", pr: 0, w: 5, conc: 4, share: 0.03, fail: 0.028 },
  { id: "cr-kimi-01", up: "up-kimi", ep: "ep-kimi", cat: "kimi", email: "km***@kimi.com", plan: null, status: "disabled", avail: "available", pr: 0, w: 2, conc: 4, share: 0, fail: 0 },
];
export const A = Object.fromEntries(accounts.map((a) => [a.id, a]));

export const quota: Record<string, S<"AccountQuotaObservation">> = {};
const win = (name: string, used: number | null, reset: number, dur: number | null, limit: number | null = null, unit: string | null = null): S<"AccountQuotaObservation">["windows"][number] => ({ name, used_percent: used, reset_at: null, reset_at_ms: NOW + reset, duration_seconds: dur, used: limit != null && used != null ? Math.round((limit * used) / 100) : null, limit, unit });
quota["cr-codex-01"] = { source: "codex", observed_at_ms: NOW - 4 * MIN, windows: [win("primary_window", 14, 2 * HOUR + 11 * MIN, 18000), win("secondary_window", 39, 3 * DAY + 4 * HOUR, 604800)], email: "li***@outlook.com", plan: "pro" };
quota["cr-codex-02"] = { source: "codex", observed_at_ms: NOW - 6 * MIN, windows: [win("primary_window", 28, 1 * HOUR + 38 * MIN, 18000), win("secondary_window", 56, 5 * DAY + 2 * HOUR, 604800)], email: "ops***@gmail.com", plan: "pro" };
quota["cr-codex-03"] = { source: "codex", observed_at_ms: NOW - 2 * MIN, windows: [win("primary_window", 100, 37 * MIN, 18000), win("secondary_window", 91, 2 * DAY + HOUR, 604800)], email: "dev***@cpar.dev", plan: "plus" };
quota["cr-claude-01"] = { source: "claude", observed_at_ms: NOW - 3 * MIN, windows: [win("five_hour", 22, 3 * HOUR + 5 * MIN, 18000), win("seven_day", 47, 4 * DAY, 604800), win("seven_day_opus", 63, 4 * DAY, 604800)], email: "cl***@proton.me", plan: "max20x" };
quota["cr-claude-02"] = { source: "claude", observed_at_ms: NOW - 9 * MIN, windows: [win("five_hour", 86, 52 * MIN, 18000), win("seven_day", 71, 2 * DAY + 3 * HOUR, 604800)], email: "ka***@gmail.com", plan: "pro" };
quota["na-grok-01"] = { source: "grok_build", observed_at_ms: NOW - 5 * MIN, windows: [win("subscription_monthly", 36, 17 * DAY, 2592000, 5000, "credits"), win("pay_as_you_go", 19, 17 * DAY, 2592000, 50, "usd")], plan: "heavy" };
quota["na-grok-02"] = { source: "grok_build", observed_at_ms: NOW - 7 * MIN, windows: [win("subscription_monthly", 73, 9 * DAY + 20 * HOUR, 2592000, 2000, "credits")], plan: "supergrok" };
quota["na-grok-03"] = { source: "grok_web", observed_at_ms: NOW - 11 * MIN, windows: [win("hourly", 90, 12 * MIN, 3600, 10, "queries"), win("daily", 70, 8 * HOUR, 86400, 30, "queries")], plan: "free", partial: true };
quota["cr-kiro-01"] = { source: "kiro", observed_at_ms: NOW - 40 * MIN, windows: [win("monthly_credits", 67, 6 * DAY + 5 * HOUR, 2592000, 1000, "credits")], plan: "pro" };

export const identity = (a: Acc): S<"AccountDisplayIdentity"> => ({ email: a.email, phone: null, username: null });
export const credential = (a: Acc): S<"Credential"> => ({ id: a.id, upstream_id: a.up, kind: a.cat === "api" ? "api_key" : a.cat + "_oauth", status: a.status, revision: 3 + (a.id.length % 5), secret_present: true });

export const bindings: S<"Binding">[] = accounts.map((a) => ({ endpoint_id: a.ep, upstream_id: a.up, credential_id: a.id, enabled: a.status !== "disabled", priority: a.pr, weight: a.w, concurrency: a.conc }));

export const publicModels: S<"PublicModel">[] = [
  { id: "pm-gpt55", model_name: "gpt-5.5", status: "active", display_name: "GPT-5.5", capabilities: { streaming: true, tools: true, reasoning: true, vision: true } },
  { id: "pm-gpt55-codex", model_name: "gpt-5.5-codex", status: "active", display_name: "GPT-5.5 Codex", capabilities: { streaming: true, tools: true, reasoning: true, vision: false } },
  { id: "pm-sonnet", model_name: "claude-sonnet-5.5", status: "active", display_name: "Claude Sonnet 5.5", capabilities: { streaming: true, tools: true, reasoning: true, vision: true } },
  { id: "pm-opus", model_name: "claude-opus-5.5", status: "active", display_name: "Claude Opus 5.5", capabilities: { streaming: true, tools: true, reasoning: true, vision: true } },
  { id: "pm-grok5", model_name: "grok-5", status: "active", display_name: "Grok 5", capabilities: { streaming: true, tools: true, reasoning: true, vision: true } },
  { id: "pm-minimax", model_name: "minimax-m3", status: "active", display_name: "MiniMax M3", capabilities: { streaming: true, tools: true, reasoning: true, vision: false } },
  { id: "pm-kimi", model_name: "kimi-k3", status: "disabled", display_name: "Kimi K3", capabilities: { streaming: true, tools: true, reasoning: true, vision: false } },
];
export const aliases: S<"Alias">[] = [{ alias: "gpt-latest", public_model_id: "pm-gpt55" }, { alias: "sonnet", public_model_id: "pm-sonnet" }, { alias: "opus", public_model_id: "pm-opus" }, { alias: "m3", public_model_id: "pm-minimax" }];
export const routes: S<"RouteListItem">[] = publicModels.filter((m) => m.status === "active").map((m) => ({ id: m.id.replace("pm-", "rt-"), public_model_id: m.id, policy: "smooth_weighted_round_robin", max_attempts: 3, bootstrap_timeout_ms: 15000 }));
const cand = (id: string, route: string, ep: string, model: string, mode: S<"Candidate">["transform_mode"], pr: number, w: number): S<"Candidate"> => ({ id, route_id: route, endpoint_id: ep, upstream_model: model, credential_scope: "all_active", transform_mode: mode, enabled: true, priority: pr, weight: w, capability_override: {} });
export const candidates: S<"Candidate">[] = [
  cand("cd-gpt55", "rt-gpt55", "ep-codex", "gpt-5.5", "passthrough", 0, 10),
  cand("cd-gpt55-codex", "rt-gpt55-codex", "ep-codex", "gpt-5.5-codex", "passthrough", 0, 10),
  cand("cd-sonnet-claude", "rt-sonnet", "ep-claude", "claude-sonnet-5-5", "passthrough", 0, 8),
  cand("cd-sonnet-kiro", "rt-sonnet", "ep-kiro", "claude-sonnet-5.5", "canonical", 1, 4),
  cand("cd-opus-claude", "rt-opus", "ep-claude", "claude-opus-5-5", "passthrough", 0, 10),
  cand("cd-grok5", "rt-grok5", "ep-grok-build", "grok-5", "passthrough", 0, 10),
  cand("cd-m3-chat", "rt-minimax", "ep-mm-chat", "MiniMax-M3", "canonical", 0, 6),
  cand("cd-m3-msg", "rt-minimax", "ep-mm-msg", "MiniMax-M3", "lossless_bridge", 0, 4),
];

export const accessGroups: S<"AccessGroup">[] = [
  { id: "grp-eng", name: "工程团队", status: "active", limits: { rpm: 1200, tpm: 4000000, concurrency: 48 } },
  { id: "grp-cc", name: "Claude Code 本地", status: "active", limits: { rpm: 300, concurrency: 8 } },
  { id: "grp-batch", name: "批处理任务", status: "active", limits: { rpm: 200, tpm: 8000000, concurrency: 16 } },
  { id: "grp-trial", name: "试用", status: "disabled", limits: { rpm: 30, concurrency: 2 } },
];
export const grants: S<"AccessGroupRoute">[] = [
  ...routes.map((r) => ({ access_group_id: "grp-eng", route_id: r.id, enabled: true })),
  ...["rt-sonnet", "rt-opus", "rt-minimax"].map((r) => ({ access_group_id: "grp-cc", route_id: r, enabled: true })),
  ...["rt-minimax", "rt-grok5"].map((r) => ({ access_group_id: "grp-batch", route_id: r, enabled: true })),
  { access_group_id: "grp-trial", route_id: "rt-minimax", enabled: true },
];
export const clientKeys: (S<"ClientKey"> & { _name: string })[] = [
  { id: "ck-alice", _name: "Alice · 笔记本", access_group_id: "grp-eng", prefix: "rgw_7Kq2", status: "active", expires_at_ms: null, last_request_at_ms: NOW - 2 * MIN },
  { id: "ck-ci", _name: "CI 夜间回归", access_group_id: "grp-batch", prefix: "rgw_Hn3x", status: "active", expires_at_ms: NOW + 23 * DAY, last_request_at_ms: NOW - 9 * HOUR },
  { id: "ck-cc-mbp", _name: "Claude Code · MBP", access_group_id: "grp-cc", prefix: "rgw_p0Lm", status: "active", expires_at_ms: NOW + 88 * DAY, last_request_at_ms: NOW - 40 * 1000 },
  { id: "ck-agent", _name: "代码审查 Agent", access_group_id: "grp-eng", prefix: "rgw_Za91", status: "active", expires_at_ms: null, last_request_at_ms: NOW - 6 * MIN },
  { id: "ck-bob", _name: "Bob · 台式机", access_group_id: "grp-eng", prefix: "rgw_c4Tt", status: "disabled", expires_at_ms: null, last_request_at_ms: NOW - 12 * DAY },
  { id: "ck-eval", _name: "外部评测", access_group_id: "grp-trial", prefix: "rgw_E2vB", status: "active", expires_at_ms: NOW - 2 * DAY, last_request_at_ms: NOW - 3 * DAY },
  { id: "ck-legacy", _name: "旧版脚本", access_group_id: "grp-batch", prefix: "rgw_0oPq", status: "revoked", expires_at_ms: null, last_request_at_ms: null },
];
export const proxyPools: S<"CompatibleProxyPool">[] = [{ id: "pool-mm", upstream_id: "up-minimax", name: "MiniMax 出口池", enabled: true }];
export const proxyNodes: S<"CompatibleProxyNode">[] = [
  { id: "node-hk-1", upstream_id: "up-minimax", pool_id: "pool-mm", name: "HK-1", enabled: true, weight: 3, maximum_concurrency: 32, proxy_configured: true },
  { id: "node-sg-1", upstream_id: "up-minimax", pool_id: "pool-mm", name: "SG-1", enabled: true, weight: 2, maximum_concurrency: 24, proxy_configured: true },
  { id: "node-jp-1", upstream_id: "up-minimax", pool_id: "pool-mm", name: "JP-1", enabled: false, weight: 1, maximum_concurrency: 16, proxy_configured: false },
];

/* ---------------- versions + audit ---------------- */
export const versions: S<"ConfigVersion">[] = [
  { id: "cv-0045", parent_id: "cv-0044", status: "active", revision: "rev-45", created_at_ms: NOW - 2 * DAY - 3 * HOUR, description: "MiniMax M3 Messages 端点改为无损桥接" },
  { id: "cv-0044", parent_id: "cv-0043", status: "archived", revision: "rev-44", created_at_ms: NOW - 4 * DAY, description: "签发 CI 批处理 Key，批处理组 TPM 上调至 8M" },
  { id: "cv-0043", parent_id: "cv-0041", status: "archived", revision: "rev-43", created_at_ms: NOW - 6 * DAY, description: "接入 Kiro 账号" },
  { id: "cv-0042", parent_id: "cv-0041", status: "archived", revision: "rev-42", created_at_ms: NOW - 8 * DAY, description: "试验：Sonnet 改用 Kiro 规范化候选（已回滚）" },
  { id: "cv-0041", parent_id: null, status: "archived", revision: "rev-41", created_at_ms: NOW - 11 * DAY, description: "绑定 Codex 第 4 个账号，权重 4" },
];
export const audit: S<"AuditEvent">[] = [
  [NOW - 2 * DAY - 3 * HOUR, "config_version.publish", "cv-0045", "cv-0044"],
  [NOW - 2 * DAY - 3.2 * HOUR, "config_version.validate", "cv-0045", null],
  [NOW - 4 * DAY, "client_key.issue", "cv-0044", null],
  [NOW - 4 * DAY + 5 * MIN, "config_version.publish", "cv-0044", "cv-0043"],
  [NOW - 6 * DAY, "config_version.publish", "cv-0043", "cv-0042"],
  [NOW - 7 * DAY, "config_version.rollback", "cv-0043", "cv-0042"],
  [NOW - 8 * DAY, "config_version.publish", "cv-0042", "cv-0041"],
].map(([t, action, cv, rep], i) => ({ id: 1240 - i, action: action as string, actor: "admin", occurred_at_ms: t as number, config_version_id: cv as string, replaced_config_version_id: rep as string | null }));
export const resourceAudit: S<"ResourceAuditPage">["items"] = [
  ["credential.status.update", "credential", "cr-codex-04", 40 * MIN], ["native_account.authorize", "native_account", "na-grok-02", 3 * HOUR], ["client_key.update", "client_key", "ck-bob", 9 * HOUR],
  ["binding.update", "binding", "ep-claude/cr-claude-02", 26 * HOUR], ["upstream.update", "upstream", "up-kimi", 2 * DAY], ["account_import", "credential", "cr-mm-02", 3 * DAY],
].map(([action, kind, id, ago], i) => ({ id: `ra-${900 - i}`, action: action as string, actor: "admin", config_version_id: "cv-0045", resource_kind: kind as string, resource_id: id as string, occurred_at_ms: NOW - (ago as number) }));

/* ---------------- requests / usage / billing ---------------- */
const MODELS: Record<string, string[]> = { "up-codex": ["gpt-5.5-codex", "gpt-5.5"], "up-claude": ["claude-sonnet-5.5", "claude-opus-5.5"], "up-grok": ["grok-5"], "up-minimax": ["minimax-m3"], "up-kiro": ["claude-sonnet-5.5"], "up-kimi": ["kimi-k3"] };
const KEYS = ["ck-alice", "ck-agent", "ck-ci", "ck-cc-mbp"];
const PRICE: Record<string, [number, number]> = { "gpt-5.5-codex": [1.25, 10], "gpt-5.5": [1.25, 10], "claude-sonnet-5.5": [3, 15], "claude-opus-5.5": [15, 75], "grok-5": [3, 15], "minimax-m3": [0.3, 1.2] };
const live = accounts.filter((a) => a.share > 0);

export type Req = S<"RequestHistoryRow">;
export const requests: Req[] = [];
export const attempts: Record<string, S<"Attempt">[]> = {};
{
  let t = NOW - 5000;
  for (let i = 0; i < 260; i++) {
    t -= Math.floor(3000 + R() * 22000);
    let r = R(), a = live[0];
    for (const x of live) { r -= x.share; if (r <= 0) { a = x; break; } }
    const model = pick(MODELS[a.up]);
    const key = a.cat === "claude" || a.cat === "kiro" ? pick(["ck-cc-mbp", "ck-alice"]) : pick(KEYS);
    const failed = R() < a.fail * 3;
    const retried = !failed && R() < 0.08;
    const dur = Math.round(900 + R() * 9000 + (failed ? 1200 : 0));
    const inT = Math.round(6000 + R() * 80000), outT = failed ? null : Math.round(200 + R() * 3000);
    const est = a.cat === "grok" && a.native === "grok_web";
    const id = "req_" + (0x1a2b3c + i * 7919).toString(36);
    const cost = failed || est ? null : Math.round((inT * PRICE[model][0] + (outT ?? 0) * PRICE[model][1]) * (a.cat === "api" ? 1 : 0.62));
    const proto = a.cat === "claude" || a.cat === "kiro" || a.ep === "ep-mm-msg" ? "anthropic_messages" : a.ep === "ep-mm-chat" ? "openai_chat_completions" : "openai_responses";
    requests.push({
      request_id: id, client_key_id: key, model, requested_model: model, protocol: proto, upstream_id: a.up, endpoint_id: a.ep, credential_id: a.id,
      error_code: failed ? pick(["ProviderRateLimited", "CredentialQuotaExceeded", "ProviderTransient", "EgressRejected"]) : null,
      started_at_ms: t, finished_at_ms: t + dur, duration_ms: dur, first_content_ms: failed ? null : Math.round(280 + R() * 2200), streaming: R() < 0.86,
      attempt_count: retried || failed ? 2 : 1, outcome: failed ? "failed" : "succeeded",
      usage: { provenance: est ? "estimated" : "measured", input_accounting: "inclusive", input_tokens: est ? null : inT, output_tokens: est ? null : outT, reasoning_tokens: est || failed ? null : Math.round(R() * 1200), cache_read_tokens: est ? null : Math.round(inT * (0.5 + R() * 0.45)), cache_creation_tokens: a.cat === "claude" ? Math.round(inT * 0.04) : 0, cached_tokens: est ? null : Math.round(inT * 0.6) },
      cost_microunits: cost, cost_confidence: failed ? "unknown" : est ? "unpriced" : a.cat === "kiro" ? "partial" : "exact", ledger_records: failed ? 0 : 1,
    });
    const peer = live.find((x) => x.ep === a.ep && x.id !== a.id && x.avail === "available") ?? a;
    const mk = (n: number, acc: Acc, outcome: string, stage: S<"Attempt">["stage"], start: number, len: number, code: S<"AttemptObservation">["error_code"], decision: S<"AttemptObservation">["retry_decision"]): S<"Attempt"> => ({ attempt_id: `${id}-a${n}`, outcome, stage, endpoint_id: acc.ep, credential_id: acc.id, observation: { attempt_number: n, upstream_id: acc.up, started_at_ms: start, ended_at_ms: start + len, duration_ms: len, error_code: code, error_scope: code ? (code === "CredentialQuotaExceeded" ? "quota_window" : code === "EgressRejected" ? "egress" : "provider") : null, retry_decision: decision } });
    attempts[id] = retried || failed
      ? [mk(1, a, "failed", "http_status", t, 300 + Math.round(R() * 400), pick(["ProviderRateLimited", "CredentialQuotaExceeded"]), "retry_eligible"), mk(2, peer, failed ? "failed" : "succeeded", failed ? "http_status" : "sse_bootstrap", t + 700, dur - 700, failed ? "ProviderTransient" : null, failed ? "retry_closed" : "completed")]
      : [mk(1, a, "succeeded", "sse_bootstrap", t, dur, null, "completed")];
  }
}

/** Hourly series for the last 7 days, scaled so 7 days ≈ 31k requests. */
export const hourly = (() => {
  const out: { t: number; req: number; fail: number; dur: number; first: number }[] = [];
  for (let i = 167; i >= 0; i--) {
    const t = Math.floor((NOW - i * HOUR) / HOUR) * HOUR; const d = new Date(t); const hr = d.getHours(), dow = d.getDay();
    const w = (0.25 + Math.max(0, Math.sin(((hr - 8) / 16) * Math.PI)) * (dow === 0 || dow === 6 ? 0.55 : 1)) * (0.85 + R() * 0.3);
    const req = Math.round(w * 290); const burst = i === 52 || i === 53 ? 0.07 : 0;
    out.push({ t, req, fail: Math.round(req * (0.007 + burst + (R() < 0.08 ? 0.01 : 0))), dur: 2100 + R() * 900 + (burst ? 2400 : 0), first: 600 + R() * 400 });
  }
  return out;
})();

/** Usage aggregation rows (OperationalUsageItem), one per account × model × key. */
export const usage: S<"OperationalUsageItem">[] = [];
{
  const total = hourly.reduce((s, h) => s + h.req, 0);
  for (const a of live) for (const m of MODELS[a.up]) for (const k of a.cat === "claude" || a.cat === "kiro" ? ["ck-cc-mbp", "ck-alice"] : ["ck-alice", "ck-agent", "ck-ci"]) {
    const req = Math.max(2, Math.round(total * a.share * (MODELS[a.up].length > 1 ? (m === MODELS[a.up][0] ? 0.65 : 0.35) : 1) * (k === "ck-alice" ? 0.5 : k === "ck-cc-mbp" ? 0.5 : 0.25)));
    const unknown = a.native === "grok_web"; const partial = a.cat === "kiro";
    const inT = Math.round(req * (a.cat === "codex" ? 52000 : a.cat === "claude" ? 61000 : 24000) * (0.85 + R() * 0.3));
    const metric = (v: number | null): S<"OperationalTokenMetric"> => ({ total: unknown ? null : v, confidence: unknown ? "unknown" : partial ? "partial" : "exact", provenance: unknown ? "unknown" : "measured" });
    usage.push({ provider_id: a.up, channel_id: a.ep, account_id: a.id, public_model: m, protocol: a.cat === "claude" || a.cat === "kiro" ? "anthropic_messages" : a.ep === "ep-mm-chat" ? "openai_chat_completions" : "openai_responses", client_key_id: k, access_group_id: clientKeys.find((c) => c.id === k)!.access_group_id,
      request_count: req, usage_observations: unknown ? 0 : req, input_tokens: metric(inT), output_tokens: metric(Math.round(req * (700 + R() * 900))), reasoning_tokens: metric(Math.round(req * (300 + R() * 500))),
      cache_read_tokens: metric(Math.round(inT * (0.62 + R() * 0.3))), cache_creation_tokens: metric(a.cat === "claude" ? Math.round(inT * 0.04) : 0), cached_tokens: metric(Math.round(inT * 0.6)), observed_at_ms: NOW - Math.round(R() * 9 * MIN), cost_microunits: null, cost_confidence: "unpriced" });
  }
}

export const billingCatalogs: S<"BillingCatalog">[] = [
  { catalog_version_id: "price-2026-10", effective_at_ms: NOW - 20 * DAY, source: "imported", created_at_ms: NOW - 20 * DAY, entries: Object.entries(PRICE).map(([model, [i, o]]) => { const a = live.find((x) => MODELS[x.up].includes(model))!; return { provider_id: a.up, channel_id: a.ep, model, input_microunits_per_million: i * 1e6, output_microunits_per_million: o * 1e6, reasoning_microunits_per_million: o * 1e6, cache_read_microunits_per_million: i * 1e5, cache_creation_microunits_per_million: i * 1.25e6, cached_microunits_per_million: i * 1e5 }; }) },
  { catalog_version_id: "price-2026-09", effective_at_ms: NOW - 51 * DAY, source: "operator", created_at_ms: NOW - 51 * DAY, entries: [] },
];
export const failures: S<"ProviderAccountFailureItem">[] = requests.filter((r) => r.outcome === "failed").slice(0, 20).map((r) => ({ provider_id: r.upstream_id!, channel_id: r.endpoint_id!, account_id: r.credential_id!, request_id: r.request_id, attempt_id: `${r.request_id}-a2`, ended_at_ms: r.finished_at_ms!, error_code: r.error_code as S<"ProviderAccountFailureItem">["error_code"], error_scope: "provider", retry_decision: "retry_closed" }) as S<"ProviderAccountFailureItem">);

export const catalogStatus: S<"CatalogStatus"> = accounts.filter((a) => a.status !== "disabled").map((a, i) => {
  const ep = endpoints.find((e) => e.id === a.ep)!; const missing = !ep.models_path; const stale = i % 5 === 3;
  return { endpoint_id: a.ep, credential_id: a.id, freshness: missing ? "missing" : a.avail === "recovery_required" ? "expired" : stale ? "stale" : "fresh", observed_at_ms: NOW - (stale ? 26 : 1 + i) * HOUR, snapshot_version: 10 + i, refresh_due: stale, model_count: missing ? 0 : 6 + i, last_failure_at_ms: NOW - 3 * DAY, last_failure_class: "rate_limit", last_success_at_ms: missing ? null : NOW - (stale ? 26 : 1 + i) * HOUR, observation_state: missing ? "unsupported" : "succeeded", source: "upstream_catalog" } as S<"CatalogStatus">[number];
});

export const METRICS = `# HELP gateway_observability_events_total Background telemetry events accepted.
# TYPE gateway_observability_events_total counter
gateway_observability_events_total 1284117
# HELP gateway_recording_batches_total SQLite append-only writer batches committed.
# TYPE gateway_recording_batches_total counter
gateway_recording_batches_total 40322
# HELP gateway_recording_queue_depth Bounded writer queue depth.
# TYPE gateway_recording_queue_depth gauge
gateway_recording_queue_depth 3
# HELP gateway_recording_dropped_total Events dropped at the bounded queue.
# TYPE gateway_recording_dropped_total counter
gateway_recording_dropped_total 0`;
export const KEY_NAMES = Object.fromEntries(clientKeys.map((k) => [k.id, k._name]));
