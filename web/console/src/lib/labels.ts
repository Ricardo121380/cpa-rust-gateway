// Contract enum values → Chinese labels and status tones. Status tones are reserved colors that always
// ship with a glyph and text; categorical identity uses --c1..--c6 instead.
export type Tone = "ok" | "warn" | "serious" | "bad" | "off" | "info";
export const GLYPH: Record<Tone, string> = { ok: "●", warn: "◐", serious: "↻", bad: "⊘", off: "○", info: "◇" };

type Map = Record<string, [string, Tone]>;
export const LABELS: Record<string, Map> = {
  availability: { available: ["可用", "ok"], cooldown: ["冷却中", "warn"], quota_blocked: ["配额耗尽", "warn"], recovery_required: ["待恢复", "serious"], circuit_open: ["熔断", "bad"], credential_forbidden: ["凭据被拒", "bad"], credential_unauthorized: ["未授权", "bad"], expired: ["已过期", "bad"] },
  runtime_status: { available: ["可用", "ok"], cooling: ["冷却中", "warn"], quota_blocked: ["配额耗尽", "warn"], recovery_in_flight: ["恢复中", "serious"], circuit_open: ["熔断", "bad"], unauthorized: ["未授权", "bad"], expired: ["已过期", "bad"] },
  auth_status: { active: ["已授权", "ok"], reauth_required: ["需重新授权", "bad"], disabled: ["已停用", "off"], expired: ["已过期", "bad"] },
  account_status: { enabled: ["启用", "ok"], disabled: ["停用", "off"], reauth_required: ["需重新授权", "bad"] },
  credential_status: { active: ["启用", "ok"], disabled: ["停用", "off"], revoked: ["已吊销", "bad"], cooling: ["冷却中", "warn"], unauthorized: ["未授权", "bad"] },
  freshness: { fresh: ["新鲜", "ok"], stale: ["陈旧", "warn"], expired: ["过期", "serious"], missing: ["缺失", "bad"] },
  confidence: { exact: ["精确", "ok"], partial: ["部分", "warn"], unknown: ["未知", "off"], unpriced: ["未定价", "off"] },
  provenance: { measured: ["实测", "ok"], estimated: ["估算", "warn"], unknown: ["未知", "off"] },
  cv_status: { draft: ["草稿", "info"], active: ["生效中", "ok"], archived: ["已归档", "off"] },
  key_status: { active: ["有效", "ok"], disabled: ["停用", "off"], revoked: ["已吊销", "bad"], expired: ["已过期", "warn"] },
  group_status: { active: ["启用", "ok"], disabled: ["停用", "off"] },
  outcome: { succeeded: ["成功", "ok"], failed: ["失败", "bad"], cancelled: ["已取消", "off"], unknown: ["未知", "off"] },
  oauth: { pending: ["等待授权", "info"], complete: ["已完成", "ok"], completed: ["已完成", "ok"], cancelled: ["已取消", "off"], failed: ["失败", "bad"], denied: ["被拒绝", "bad"], expired: ["已过期", "warn"] },
  change: { added: ["新增", "ok"], removed: ["删除", "bad"], changed: ["修改", "warn"] },
  test: { pass: ["通过", "ok"], rejected: ["被拒绝", "bad"], transport_failed: ["传输失败", "bad"], protocol_failed: ["协议失败", "bad"] },
};

export const label = (map: string, key: string | null | undefined): [string, Tone] => (key ? LABELS[map]?.[key] ?? [key, "off"] : ["—", "off"]);

export const PROTOCOL: Record<string, string> = { openai_chat_completions: "Chat Completions", openai_responses: "Responses", anthropic_messages: "Messages" };
export const STAGE: Record<string, string> = { request_conversion: "请求转换", egress_admission: "出口准入", http_transport: "HTTP 传输", http_status: "HTTP 状态", content_type: "内容类型", body_read: "读取正文", decoder: "解码", sse_bootstrap: "SSE 启动" };
export const TRANSFORM: Record<string, string> = { passthrough: "直通", canonical: "规范化", lossless_bridge: "无损桥接", canonical_bridge: "规范桥接" };
export const CATEGORY: Record<string, string> = { api: "API Key", codex: "Codex", claude: "Claude", kimi: "Kimi", kiro: "Kiro", grok: "Grok" };

/** Stable categorical slot per account category / provider family (fixed order, never cycled). */
const SLOT: Record<string, number> = { codex: 1, grok: 2, claude: 3, api: 4, kiro: 5, kimi: 6 };
export const slotOf = (category: string | null | undefined) => SLOT[category ?? ""] ?? 4;
export function categoryOfKind(kind: string | null | undefined): string {
  const k = (kind ?? "").toLowerCase();
  if (k.includes("codex") || k.includes("openai_oauth")) return "codex";
  if (k.includes("grok")) return "grok";
  if (k.includes("claude")) return "claude";
  if (k.includes("kiro")) return "kiro";
  if (k.includes("kimi")) return "kimi";
  return "api";
}
