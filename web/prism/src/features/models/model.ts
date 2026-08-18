// Public-model pure model. Capabilities use the backend's frozen semantic
// capability keys; parallel_tools implies tools (mirrored client-side).
export type PublicModel = Readonly<{
  id: string;
  model_name: string;
  status: "active" | "disabled";
  display_name: string;
  capabilities: Readonly<Record<string, boolean>>;
}>;

export const SEMANTIC_CAPABILITIES = [
  "streaming",
  "tools",
  "parallel_tools",
  "reasoning",
  "json_schema",
  "vision",
] as const;

export type SemanticCapability = (typeof SEMANTIC_CAPABILITIES)[number];

/** parallel_tools ⇒ tools; turning tools off drops parallel_tools. */
export function toggleCapability(
  current: Readonly<Record<string, boolean>>,
  capability: SemanticCapability,
  enabled: boolean,
): Record<string, boolean> {
  const next: Record<string, boolean> = { ...current };
  if (enabled) {
    next[capability] = true;
    if (capability === "parallel_tools") {
      next["tools"] = true;
    }
  } else {
    delete next[capability];
    if (capability === "tools") {
      delete next["parallel_tools"];
    }
  }
  return next;
}

export function enabledCapabilities(
  capabilities: Readonly<Record<string, boolean>>,
): string[] {
  return Object.entries(capabilities)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name)
    .sort();
}

export const ROUTE_POLICY = "smooth_weighted_round_robin" as const;

export function validRouteParams(maxAttempts: number, bootstrapTimeoutMs: number): boolean {
  return (
    Number.isInteger(maxAttempts) &&
    maxAttempts >= 1 &&
    maxAttempts <= 16 &&
    Number.isInteger(bootstrapTimeoutMs) &&
    bootstrapTimeoutMs >= 1 &&
    bootstrapTimeoutMs <= 120000
  );
}

// ---------------------------------------------------------------------------
// route candidates — POST /admin/routes/{route_id}/candidates
// ---------------------------------------------------------------------------
//
// Candidates are INSERT-ONLY in the contract: there is no list, no update and
// no delete operation for them. The only read path is explainRoute, which
// needs a requested_model and a protocol to answer. That asymmetry is a
// contract fact, not an omission here — every surface below states it rather
// than implying a candidate can be edited back out.

export type RouteRecord = Readonly<{
  id: string;
  public_model_id: string;
  policy: typeof ROUTE_POLICY;
  max_attempts: number;
  bootstrap_timeout_ms: number;
}>;

export type RouteValidation = Readonly<{
  valid: boolean;
  error_codes?: readonly string[];
}>;

/** The four transform modes, in the contract's own order. */
export const TRANSFORM_MODES = [
  "passthrough",
  "canonical",
  "lossless_bridge",
  "canonical_bridge",
] as const;

export type TransformMode = (typeof TRANSFORM_MODES)[number];

/** The contract's credential_scope enum currently has exactly one member. */
export const CREDENTIAL_SCOPE = "all_active" as const;

const TRANSFORM_MODE_HINT: Readonly<Record<TransformMode, string>> = {
  passthrough: "原样转发,不做协议转换",
  canonical: "经 Canonical 中间表示往返",
  lossless_bridge: "受限无损桥接到另一协议",
  canonical_bridge: "Canonical + 桥接组合",
};

export function transformModeHint(mode: TransformMode): string {
  return TRANSFORM_MODE_HINT[mode];
}

export function validCandidateParams(priority: number, weight: number): boolean {
  return (
    Number.isInteger(priority) &&
    priority >= 0 &&
    Number.isInteger(weight) &&
    weight >= 1 &&
    weight <= 10000
  );
}

// capability_override is a REQUIRED free-form Record<string, boolean> with at
// most 32 entries — `{}` is the valid "override nothing" value. The keys are
// not restricted to SEMANTIC_CAPABILITIES by the contract, so a checkbox grid
// over our own list would silently refuse keys the backend accepts. Free text
// in the same `key=value` shape as access-group limits keeps arbitrary keys
// expressible and matches a format operators already read elsewhere.
const MAX_CAPABILITY_OVERRIDES = 32;

export function formatCapabilityOverride(
  override: Readonly<Record<string, boolean>>,
): string {
  return Object.entries(override)
    .map(([key, value]) => `${key}=${value ? "true" : "false"}`)
    .join(" ");
}

export type ParsedCapabilityOverride =
  | Readonly<{ ok: true; override: Readonly<Record<string, boolean>> }>
  | Readonly<{ ok: false; reason: string }>;

export function parseCapabilityOverride(raw: string): ParsedCapabilityOverride {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: true, override: {} };
  }
  const override: Record<string, boolean> = {};
  for (const token of trimmed.split(/\s+/u)) {
    const at = token.indexOf("=");
    if (at <= 0 || at === token.length - 1) {
      return { ok: false, reason: `「${token}」不是 key=true 或 key=false 形式。` };
    }
    const key = token.slice(0, at);
    const rawValue = token.slice(at + 1);
    if (Object.hasOwn(override, key)) {
      return { ok: false, reason: `能力键「${key}」重复。` };
    }
    if (rawValue !== "true" && rawValue !== "false") {
      return {
        ok: false,
        reason: `「${key}」的值只能是 true 或 false,契约不接受 ${rawValue}。`,
      };
    }
    override[key] = rawValue === "true";
  }
  if (Object.keys(override).length > MAX_CAPABILITY_OVERRIDES) {
    return { ok: false, reason: `能力覆盖最多 ${MAX_CAPABILITY_OVERRIDES} 项。` };
  }
  return { ok: true, override };
}

// The four codes validate_model_route actually emits
// (gateway-control/src/management_mutation_service.rs). An unknown code is
// rendered as itself: the backend may add codes, and a guessed translation
// would be worse than the raw string an operator can grep for.
const ROUTE_ERROR_LABEL: Readonly<Record<string, string>> = {
  route_missing_active_candidate: "路由没有任何启用的候选 —— 建路由后必须至少加一个候选",
  route_candidate_endpoint_missing: "候选指向的端点在本版本中不存在",
  route_candidate_endpoint_disabled: "候选指向的端点被禁用",
  route_candidate_missing_active_credential:
    "候选端点没有启用且状态为 active 的凭据绑定",
};

export function routeErrorLabel(code: string): string | undefined {
  return ROUTE_ERROR_LABEL[code];
}

