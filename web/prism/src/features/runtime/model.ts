// Runtime projections — pure model (docs/07 §7.5).
//
// Four real contract operations feed this page, all of them INJECTED FACADES in
// the gateway that fail closed. So every shape here is defensive in one precise
// way: the state vocabularies are frozen closed enums, but an unknown member is
// rendered honestly (as itself, muted) instead of being coerced into a
// neighbouring state. We never invent a status the backend did not send.
//
// Nothing in this file touches the DOM, the clock or the network: `nowMs` is
// always a parameter so the whole module is testable in node.

/** Badge/tone vocabulary shared with StatusBadge (docs/07 §8.6). */
export type Tone = "good" | "warn" | "serious" | "critical" | "tint" | "muted";

// ---------------------------------------------------------------------------
// availability — GET /admin/runtime/availability
// ---------------------------------------------------------------------------

/** Contract availability states; order is display order. */
export const AVAILABILITY_STATES = [
  "available",
  "cooldown",
  "circuit_open",
  "quota_blocked",
  "credential_forbidden",
  "recovery_required",
  "credential_unauthorized",
  "expired",
] as const;

export type AvailabilityState = (typeof AVAILABILITY_STATES)[number];

export type AvailabilityRow = Readonly<{
  endpoint_id: string;
  credential_id: string;
  availability: string;
}>;

export type StateMeta = Readonly<{
  /** short cell label — Chinese, so the cell never relies on colour alone */
  label: string;
  /** shape redundancy: a distinct glyph per state, not six identical dots */
  glyph: string;
  tone: Tone;
  /** one-line semantics, shown in the legend and as the cell tooltip */
  detail: string;
  /**
   * English for THIS vocabulary.
   *
   * It lives beside the enum rather than in the i18n pack because the pack is a
   * flat key space and these vocabularies deliberately overlap: `disabled` is a
   * credential an operator switched off, a price comparison that never ran, AND
   * an administratively disabled egress. `active`, `expired`, `fresh` and
   * `available` collide the same way. One flat table would merge exactly the
   * distinctions the whole module exists to keep apart — see authStatusMeta,
   * which must answer 未知 for a runtime-axis value.
   *
   * The pack still owns page chrome; enum vocabulary is data, and data travels
   * with its enum. i18n.test.ts fails if any entry here lacks an `en`.
   */
  en?: Readonly<{ label: string; detail: string }>;
}>;

const AVAILABILITY_META: Readonly<Record<AvailabilityState, StateMeta>> = {
  credential_unauthorized: {
    label: "待重授权", glyph: "!", tone: "critical",
    detail: "凭据需要重新授权或替换", en: { label: "Reauthorization required", detail: "Reauthorize or replace the credential" },
  },
  expired: {
    label: "已过期", glyph: "◷", tone: "serious",
    detail: "当前凭据已过期，等待有效续期", en: { label: "Expired", detail: "Credential expired; waiting for valid renewal" },
  },
  available: {
    label: "可用",
    glyph: "●",
    tone: "good",
    detail: "绑定级健康及配额未阻断；模型准入和并发另行判定", en: { label: "Available", detail: "No binding health/quota block; model eligibility and concurrency are evaluated separately" },
  },
  cooldown: {
    label: "冷却",
    glyph: "◐",
    tone: "warn",
    detail: "瞬时冷却,到期自动恢复", en: { label: "Cooling", detail: "Momentary cooldown; returns on its own" },
  },
  circuit_open: {
    label: "熔断",
    glyph: "▲",
    tone: "serious",
    detail: "连续失败触发熔断,等待半开探测", en: { label: "Circuit open", detail: "Repeated failures tripped the circuit; awaiting a half-open probe" },
  },
  quota_blocked: {
    label: "配额",
    glyph: "◼",
    tone: "serious",
    detail: "配额窗耗尽,等待窗口重置", en: { label: "Quota spent", detail: "Quota window exhausted; waiting for the window to reset" },
  },
  credential_forbidden: {
    label: "禁用",
    glyph: "✕",
    tone: "critical",
    detail: "凭据被上游拒绝(有证据),需受控恢复", en: { label: "Refused", detail: "Upstream refused this credential (with evidence); needs controlled recovery" },
  },
  recovery_required: {
    label: "待恢复",
    glyph: "↻",
    tone: "tint",
    detail: "已登记恢复需求,等待人工或探测放行", en: { label: "Recovery pending", detail: "Recovery is registered but not admitted — needs a person or a probe" },
  },
};

const UNKNOWN_META: StateMeta = {
  label: "未知",
  glyph: "?",
  tone: "muted",
  detail: "契约未声明的状态值 —— 原样呈现,不做归并",
  en: { label: "Unknown", detail: "A value the contract does not declare — shown as itself, never merged" },
};

/** Pick the language's text without touching glyph or tone, which are not
 *  language-dependent. A missing `en` falls through to the Chinese rather than
 *  to an empty string: a visibly untranslated label is honest, a blank one is
 *  a bug that looks like a rendering fault. */
export function localizeMeta(meta: StateMeta, lang: string): StateMeta {
  if (lang !== "en" || meta.en === undefined) {
    return meta;
  }
  return { ...meta, label: meta.en.label, detail: meta.en.detail };
}

export function isAvailabilityState(value: string): value is AvailabilityState {
  return (AVAILABILITY_STATES as readonly string[]).includes(value);
}

export function availabilityMeta(value: string): StateMeta {
  return isAvailabilityState(value) ? AVAILABILITY_META[value] : UNKNOWN_META;
}

/** data-state attribute for CSS: unknown members collapse to one bucket. */
export function stateAttr(value: string): string {
  return isAvailabilityState(value) ? value : "unknown";
}

/** Composite key for the sparse endpoint × credential grid. */
export function cellKey(endpointId: string, credentialId: string): string {
  return `${endpointId}\0${credentialId}`;
}

export type AvailabilityMatrix = Readonly<{
  endpoints: readonly string[];
  credentials: readonly string[];
  cells: ReadonlyMap<string, AvailabilityRow>;
}>;

/**
 * Axes are the sorted union of what the projection actually returned — the
 * matrix never claims a pair exists. A missing cell is "no projection for this
 * pair", which the page renders differently from any of the six states.
 */
export function buildAvailabilityMatrix(
  rows: readonly AvailabilityRow[],
): AvailabilityMatrix {
  const endpoints = new Set<string>();
  const credentials = new Set<string>();
  const cells = new Map<string, AvailabilityRow>();
  for (const row of rows) {
    endpoints.add(row.endpoint_id);
    credentials.add(row.credential_id);
    cells.set(cellKey(row.endpoint_id, row.credential_id), row);
  }
  return {
    endpoints: [...endpoints].sort(),
    credentials: [...credentials].sort(),
    cells,
  };
}

/** Rows the operator would want to look at first. */
export function abnormalRows(
  rows: readonly AvailabilityRow[],
): readonly AvailabilityRow[] {
  return rows.filter((row) => row.availability !== "available");
}

export type StateCount = Readonly<{ state: string; count: number }>;

/** Non-zero counts, in enum order, with unknown members appended. */
export function countByState(rows: readonly AvailabilityRow[]): readonly StateCount[] {
  const tally = new Map<string, number>();
  for (const row of rows) {
    tally.set(row.availability, (tally.get(row.availability) ?? 0) + 1);
  }
  const known = AVAILABILITY_STATES.filter((state) => tally.has(state)).map((state) => ({
    state: state as string,
    count: tally.get(state) ?? 0,
  }));
  const unknown = [...tally.keys()]
    .filter((state) => !isAvailabilityState(state))
    .sort()
    .map((state) => ({ state, count: tally.get(state) ?? 0 }));
  return [...known, ...unknown];
}

// ---------------------------------------------------------------------------
// quota recovery — POST /admin/runtime/quota/reset
// ---------------------------------------------------------------------------

/** Only these two states have a recovery entry point. */
const RECOVERABLE: ReadonlySet<string> = new Set([
  "credential_forbidden",
  "quota_blocked",
]);

export function isRecoverable(availability: string): boolean {
  return RECOVERABLE.has(availability);
}

/** Recoverable pairs, in a stable display order. */
export function recoverableRows(
  rows: readonly AvailabilityRow[],
): readonly AvailabilityRow[] {
  return rows
    .filter((row) => isRecoverable(row.availability))
    .slice()
    .sort(
      (a, b) =>
        a.endpoint_id.localeCompare(b.endpoint_id) ||
        a.credential_id.localeCompare(b.credential_id),
    );
}

export const RECOVERY_STATES = [
  "released",
  "recovery_required",
  "probe_scheduled",
  "rejected",
] as const;

export type RecoveryState = (typeof RECOVERY_STATES)[number];

export type RecoveryResponse = Readonly<{ state: string }>;

/**
 * A released local block permits another attempt without proving upstream
 * availability. An accepted recovery request does not prove a probe was sent.
 */
export function recoveryMeta(state: string): StateMeta {
  switch (state) {
    case "released": return {label:"允许再次尝试，尚未验证",glyph:"↻",tone:"tint",detail:"已解除本地阻塞，没有发送调用或证明上游可用"};
    case "probe_scheduled":
      return {
        label: "恢复操作尚未完成",
        glyph: "↻",
        tone: "tint",
        detail: "请求已接收，恢复尚未完成；本回执不证明发送了上游探测",
      };
    case "recovery_required":
      return {
        label: "仍需恢复",
        glyph: "!",
        tone: "tint",
        detail: "已登记需求,但未放行 —— 需受控恢复",
      };
    case "rejected":
      return {
        label: "已拒绝",
        glyph: "✕",
        tone: "serious",
        detail: "本次部署拒绝该恢复请求",
      };
    default:
      return UNKNOWN_META;
  }
}

// ---------------------------------------------------------------------------
// catalog freshness — GET /admin/catalog/status
// ---------------------------------------------------------------------------

export const FRESHNESS_STATES = ["fresh", "stale", "expired", "missing"] as const;
export type Freshness = (typeof FRESHNESS_STATES)[number];

export type CatalogRow = Readonly<{
  endpoint_id: string;
  credential_id: string;
  freshness: string;
  observed_at_ms: number;
  last_success_at_ms?: number | null;
  observation_state?: string;
  source?: string;
  snapshot_version?: number;
  refresh_due?: boolean;
  model_count?: number;
  last_failure_at_ms?: number;
  last_failure_class?: string;
}>;

const FRESHNESS_META: Readonly<Record<Freshness, StateMeta>> = {
  fresh: { label: "新鲜", glyph: "●", tone: "good", detail: "在 6 小时新鲜期内", en: { label: "Fresh", detail: "Observed within the freshness window" },},
  stale: { label: "陈旧", glyph: "◐", tone: "warn", detail: "超过 6 小时新鲜期；24 小时后应刷新", en: { label: "Stale", detail: "Past the freshness window; refresh due after 24 hours" },},
  expired: { label: "过期", glyph: "▲", tone: "serious", detail: "超过 72 小时硬过期上限", en: { label: "Expired", detail: "Too old to be trusted" },},
  missing: { label: "缺失", glyph: "○", tone: "muted", detail: "尚无该组合的目录观测", en: { label: "Never observed", detail: "No observation has ever been recorded" },},
};

export function freshnessMeta(value: string): StateMeta {
  return (FRESHNESS_META as Record<string, StateMeta | undefined>)[value] ?? UNKNOWN_META;
}

/** Lifecycle thresholds (docs/07 §7.5) — the help text reads them from here. */
export const CATALOG_FRESH_MS = 6 * 3_600_000;
export const CATALOG_REFRESH_DUE_MS = 24 * 3_600_000;
export const CATALOG_HARD_EXPIRY_MS = 72 * 3_600_000;
/** Removal needs BOTH conditions — three misses alone never deletes an entry. */
export const CATALOG_REMOVAL_MISSES = 3;
export const CATALOG_REMOVAL_ISOLATION_MS = 24 * 3_600_000;

export type AgeStage = "unobserved" | "fresh" | "aging" | "refresh_due" | "hard_expired";

/**
 * Age bucket derived from the observation timestamp — reported ALONGSIDE the
 * server's own `freshness`, never instead of it. When the two disagree the
 * operator should see both (the backend owns the verdict; this is the clock).
 * Negative ages (clock skew between panel and gateway) clamp to fresh.
 */
export function ageStage(observedAtMs: number, nowMs: number): AgeStage {
  if (!Number.isFinite(observedAtMs) || observedAtMs <= 0) {
    return "unobserved";
  }
  const age = Math.max(0, nowMs - observedAtMs);
  if (age < CATALOG_FRESH_MS) {
    return "fresh";
  }
  if (age < CATALOG_REFRESH_DUE_MS) {
    return "aging";
  }
  if (age < CATALOG_HARD_EXPIRY_MS) {
    return "refresh_due";
  }
  return "hard_expired";
}

const AGE_STAGE_LABEL: Readonly<Record<AgeStage, string>> = {
  unobserved: "从未观测",
  fresh: "新鲜期内(<6h)",
  aging: "已过新鲜期(6-24h)",
  refresh_due: "应刷新(24-72h)",
  hard_expired: "已达硬过期(≥72h)",
};

export function ageStageLabel(stage: AgeStage): string {
  return AGE_STAGE_LABEL[stage];
}

/** Relative age in the panel's voice: coarse, never fake-precise. */
export function formatAge(observedAtMs: number, nowMs: number): string {
  if (!Number.isFinite(observedAtMs) || observedAtMs <= 0) {
    return "从未观测";
  }
  const age = nowMs - observedAtMs;
  if (age < 0) {
    return "刚刚";
  }
  const minutes = Math.floor(age / 60_000);
  if (minutes < 1) {
    return "刚刚";
  }
  if (minutes < 60) {
    return `${minutes} 分钟前`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 48) {
    return `${hours} 小时前`;
  }
  return `${Math.floor(hours / 24)} 天前`;
}

/** UTC, second precision — the same wall clock the gateway logs use. */
export function formatObservedAt(observedAtMs: number): string {
  if (!Number.isFinite(observedAtMs) || observedAtMs <= 0) {
    return "—";
  }
  return `${new Date(observedAtMs).toISOString().slice(0, 19).replace("T", " ")}Z`;
}

// ---------------------------------------------------------------------------
// route explain — GET /admin/routes/{route_id}/explain
// ---------------------------------------------------------------------------

/** The three wire protocols the contract's explain enum accepts.
 *
 *  openai_chat_completions was missing here until 2026-08-18: it landed in the
 *  contract with P12-08 and nothing failed, because the drift gate compares the
 *  vendored contract against the generated client — not against the literals a
 *  page happens to offer. Explain simply could not be run for the Chat
 *  Completions path. */
export const PROTOCOLS = [
  "openai_chat_completions",
  "openai_responses",
  "anthropic_messages",
] as const;
export type Protocol = (typeof PROTOCOLS)[number];

export type ExplainQuery = Readonly<{
  route_id: string;
  requested_model: string;
  protocol: Protocol;
  /** Optional by contract. REQUIRED in practice for a multi-Provider route:
   *  omitting it there fails closed with `provider_scope_required` (P13-07B). */
  provider_id?: string;
}>;

export type ExplainCandidate = Readonly<{
  candidate_id: string;
  decision: string;
  reason?: string | null;
  /** Required since P13-07D. One closed value per candidate, computed by the
   *  backend against the Config-Version-bound catalog — Prism never prices. */
  price_evidence: string;
}>;

/** Required, nullable: `null` means the routing price policy is disabled —
 *  which is NOT the same as "no catalog" or "cost unknown". */
export type PricePolicyLineage = Readonly<{
  catalog_version_id: string;
  comparison: string;
}>;

export type RouteExplain = Readonly<{
  route_id: string;
  price_policy: PricePolicyLineage | null;
  candidates: readonly ExplainCandidate[];
}>;

export function decisionMeta(decision: string): StateMeta {
  switch (decision) {
    case "selected":
      return { label: "选中", glyph: "●", tone: "good", detail: "本次请求会走这条候选" };
    case "excluded":
      return { label: "排除", glyph: "○", tone: "muted", detail: "被排除,原因见下一列" };
    default:
      return UNKNOWN_META;
  }
}

/** The seven `rate_dominance_v1` outcomes. These describe a RATE comparison,
 *  not a request cost: the backend compares six per-million rates and reports
 *  the partial order. "dominant" means no other candidate is cheaper on every
 *  dimension — it does not mean this request will be cheapest. */
const PRICE_EVIDENCE_META: Readonly<Record<string, StateMeta>> = {
  dominant: {
    label: "占优",
    glyph: "↓",
    tone: "good",
    detail: "六维费率上没有更便宜的候选", en: { label: "Cheapest", detail: "Cheaper than every comparable candidate on every rate" },
  },
  equal: {
    label: "持平",
    glyph: "=",
    tone: "tint",
    detail: "费率与被比较的候选完全相同", en: { label: "Tied", detail: "Identical rates to the candidate it was compared with" },
  },
  dominated: {
    label: "被占优",
    glyph: "↑",
    tone: "warn",
    detail: "存在一条候选在六维上都不更贵、且至少一维更便宜", en: { label: "Dearer", detail: "At least one comparable candidate is cheaper on every rate" },
  },
  incomparable: {
    label: "不可比",
    glyph: "⇅",
    tone: "tint",
    detail: "有的维度更便宜、有的更贵,偏序无法判定", en: { label: "Incomparable", detail: "Cheaper on some rates and dearer on others — no ordering exists" },
  },
  unpriced: {
    label: "无价格",
    glyph: "?",
    tone: "warn",
    detail: "绑定的目录里没有这条候选的费率 —— 不是零成本", en: { label: "Unpriced", detail: "No catalog entry — a gap, never free" },
  },
  not_evaluated: {
    label: "未参与比较",
    glyph: "·",
    tone: "muted",
    detail: "策略生效,但这条候选未进入本次比较", en: { label: "Not evaluated", detail: "No comparison was run for this candidate" },
  },
  disabled: {
    label: "策略关闭",
    glyph: "⊘",
    tone: "muted",
    detail: "路由价格策略未启用,没有做任何费率比较", en: { label: "Comparison off", detail: "No price policy is in force, so nothing was compared" },
  },
};

export function priceEvidenceMeta(evidence: string): StateMeta {
  return PRICE_EVIDENCE_META[evidence] ?? UNKNOWN_META;
}

/** Closed reasons Explain can fail with once a Route spans Providers. Kept
 *  apart from the generic error path so the panel can say what to DO. */
export function explainScopeHint(code: string | undefined): string | undefined {
  switch (code) {
    case "provider_scope_required":
      return "这条路由跨多个 Provider,必须显式选一个 —— 省略时后端按契约失败关闭,不会替你猜。";
    case "provider_mismatch":
      return "所选 Provider 不在这条路由的候选里。清空该字段可看它到底覆盖哪些 Provider。";
    default:
      return undefined;
  }
}

export function normalizeExplainQuery(raw: ExplainQuery): ExplainQuery | undefined {
  const route_id = raw.route_id.trim();
  const requested_model = raw.requested_model.trim();
  if (route_id === "" || requested_model === "") {
    return undefined;
  }
  const provider_id = raw.provider_id?.trim() ?? "";
  return {
    route_id,
    requested_model,
    protocol: raw.protocol,
    // Empty means "omit the parameter", not "match the empty Provider".
    ...(provider_id === "" ? {} : { provider_id }),
  };
}

export function explainCounts(
  candidates: readonly ExplainCandidate[],
): Readonly<{ selected: number; excluded: number; other: number }> {
  let selected = 0;
  let excluded = 0;
  let other = 0;
  for (const candidate of candidates) {
    if (candidate.decision === "selected") {
      selected += 1;
    } else if (candidate.decision === "excluded") {
      excluded += 1;
    } else {
      other += 1;
    }
  }
  return { selected, excluded, other };
}

// ---------------------------------------------------------------------------
// projection availability
// ---------------------------------------------------------------------------

/**
 * All four operations are injected facades that FAIL CLOSED: a deployment that
 * did not wire them answers 503, which is a first-class "this projection is not
 * enabled here" — categorically different from "enabled and empty".
 */
export function isProjectionUnavailable(error: unknown): boolean {
  if (typeof error !== "object" || error === null) {
    return false;
  }
  const candidate = error as { kind?: unknown; status?: unknown };
  return candidate.kind === "unavailable" || candidate.status === 503;
}

// ---------------------------------------------------------------------------
// provider account pools — GET /admin/operations/provider-account-pools
//                          POST .../actions                        (P13-06B/C)
// ---------------------------------------------------------------------------
//
// A SCOPE SPLIT that will bite anyone who assumes one rule for the pair:
//   the LIST declares no X-Config-Version — it is live runtime state;
//   the ACTION requires one.
// So the table reads fine with nothing selected in the top bar and every
// button is then unusable. The card says that rather than letting an operator
// discover it from a failed POST.
//
// The action carries NO If-Match: it is version-scoped but not
// revision-guarded, because it acts on runtime rather than on configuration.

/** Whether the account can authenticate at all. Independent of runtime state. */
export const AUTH_STATUSES = ["active", "reauth_required", "disabled", "expired"] as const;
export type AuthStatus = (typeof AUTH_STATUSES)[number];

/** What the scheduler is currently doing with it. */
export const RUNTIME_STATUSES = [
  "available",
  "cooling",
  "circuit_open",
  "quota_blocked",
  "unauthorized",
  "recovery_in_flight",
  "expired",
] as const;
export type RuntimeStatus = (typeof RUNTIME_STATUSES)[number];

const AUTH_META: Readonly<Record<string, StateMeta>> = {
  active: { label: "可用", glyph: "●", tone: "good", detail: "凭据可以认证", en: { label: "Usable", detail: "The credential can authenticate" },},
  reauth_required: {
    label: "待重授权",
    glyph: "◑",
    tone: "warn",
    detail: "需要人工重新授权,自动重试不会解决", en: { label: "Reauth needed", detail: "Needs a person to re-authorise; retrying will not fix it" },
  },
  disabled: { label: "已停用", glyph: "○", tone: "muted", detail: "被运维显式停用", en: { label: "Disabled", detail: "Explicitly disabled by an operator" },},
  expired: { label: "已过期", glyph: "✕", tone: "critical", detail: "凭据过期,已排除出调度", en: { label: "Expired", detail: "Credential expired; excluded from scheduling" },},
};

const RUNTIME_META: Readonly<Record<string, StateMeta>> = {
  available: { label: "可用", glyph: "●", tone: "good", detail: "参与调度", en: { label: "Available", detail: "In scheduling" },},
  cooling: { label: "冷却中", glyph: "◐", tone: "warn", detail: "临时退避,到期自动回到调度", en: { label: "Cooling", detail: "Backing off; returns to scheduling when it expires" },},
  circuit_open: { label: "熔断", glyph: "▲", tone: "serious", detail: "连续失败触发熔断", en: { label: "Circuit open", detail: "Repeated failures tripped the circuit" },},
  quota_blocked: { label: "配额耗尽", glyph: "◼", tone: "serious", detail: "配额窗用尽,等待重置", en: { label: "Quota spent", detail: "Quota window exhausted; waiting for a reset" },},
  unauthorized: {
    label: "被拒绝",
    glyph: "✕",
    tone: "critical",
    detail: "上游拒绝了这个账号 —— 这是停止,不是等待", en: { label: "Refused", detail: "The upstream refused this account — a stop, not a wait" },
  },
  recovery_in_flight: { label: "恢复中", glyph: "◇", tone: "tint", detail: "恢复探测已排程", en: { label: "Recovering", detail: "A recovery probe is scheduled" },},
  expired: { label: "已过期", glyph: "✕", tone: "critical", detail: "运行时视其为过期", en: { label: "Expired", detail: "The runtime treats it as expired" },},
};

/** The two vocabularies are INDEPENDENT AXES and are never combined into one
 *  "health" value: an account can be auth-active and runtime-cooling, or
 *  auth-expired while the runtime has not noticed yet. Collapsing them would
 *  invent a state the backend never reported. */
export function authStatusMeta(status: string): StateMeta {
  return AUTH_META[status] ?? UNKNOWN_META;
}

export function runtimeStatusMeta(status: string): StateMeta {
  return RUNTIME_META[status] ?? UNKNOWN_META;
}

export type ProviderAccountEntitlement = Readonly<{
  domain: string;
  tier: string;
  source: string;
  confidence: string;
  observed_at_ms: number;
}>;

export type PoolAccount = Readonly<{
  presentation?: Readonly<{identity: import("../accounts/presentation").AccountIdentity; category:string; provider:string; api_format:string; host:string|null; source:string|null}> | null;
  provider_id: string;
  channel_id: string;
  account_id: string;
  account_kind: string;
  credential_revision?: number | null;
  config_version_id?: string | null;
  auth_status: string;
  runtime_status: string;
  enabled: boolean;
  priority: number;
  weight: number;
  max_concurrency: number;
  active_leases: number;
  expires_at_ms: number | null;
  refresh_due_at_ms: number | null;
  quota_sync_due_at_ms: number | null;
  entitlement: ProviderAccountEntitlement | null;
}>;

export type PoolSnapshot = Readonly<{
  snapshot_id: string;
  observed_at_ms: number;
  items: readonly PoolAccount[];
  next_cursor: string | null;
}>;

/** Contract bounds for the cooldown, mirrored so the form can reject locally. */
export const COOLDOWN_MIN_MS = 1000;
export const COOLDOWN_MAX_MS = 86_400_000;

export function validCooldown(ms: number): boolean {
  return Number.isInteger(ms) && ms >= COOLDOWN_MIN_MS && ms <= COOLDOWN_MAX_MS;
}

export const POOL_ACTIONS = ["cool_down", "request_recovery"] as const;
export type PoolAction = (typeof POOL_ACTIONS)[number];

export type ActionReceipt = Readonly<{
  state: string;
  observed_at_ms: number;
  cooldown_until_ms: number | null;
  audit_recorded?:boolean;
}>;

/** Bounded receipt states. `rejected` is a real answer — the scheduler
 *  declined the request — not a transport failure, so it is not painted as an
 *  error. */
const RECEIPT_META: Readonly<Record<string, StateMeta>> = {
  released: {label:"允许再次尝试，尚未验证",glyph:"↻",tone:"tint",detail:"只解除本地阻塞；授权、额度和模型权限仍须满足",en:{label:"Retry allowed, unverified",detail:"Only the local block was released; authentication, quota and model permissions still apply"}},
  cooling: { label: "已进入冷却", glyph: "◐", tone: "warn", detail: "该账号已退出调度直到冷却到期", en: { label: "Cooling started", detail: "This account has left scheduling until the cooldown ends" },},
  probe_scheduled: {
    label: "恢复操作尚未完成",
    glyph: "◇",
    tone: "tint",
    detail: "解除请求已接收；当前恢复状态尚未完成，不证明本次向上游发送了探测", en: { label: "Recovery pending", detail: "The release request was accepted; recovery is still pending and does not prove a probe was sent upstream" },
  },
  recovery_required: {
    label: "需要人工恢复",
    glyph: "◑",
    tone: "serious",
    detail: "自动恢复不适用 —— 需要重新授权或运维介入", en: { label: "Needs a person", detail: "Automatic recovery does not apply — re-authorise or intervene" },
  },
  rejected: {
    label: "被拒绝",
    glyph: "○",
    tone: "muted",
    detail: "调度器拒绝了这次操作;这是一个答复,不是失败", en: { label: "Declined", detail: "The scheduler declined this action; that is an answer, not a failure" },
  },
};

export function receiptMeta(state: string): StateMeta {
  return RECEIPT_META[state] ?? UNKNOWN_META;
}

/** A due-time that has not been reported is not "now" and not "never" — it is
 *  unreported, and reads as such. */
export function formatDue(ms: number | null, nowMs: number): string {
  if (ms === null) {
    return "—";
  }
  return ms <= nowMs ? "已到期" : formatAge(nowMs, ms).replace("前", "后");
}

// ---------------------------------------------------------------------------
// provider egress status — GET /admin/operations/provider-egress-status
// (P13-11E4). Read-only. No action exists on this projection and none is
// invented here.
//
// THREE DOMAINS, THREE VOCABULARIES. The backend keeps one closed union of 14
// state values and checks domain compatibility explicitly, so `fresh` is a
// clearance state and never an egress one. The three maps below reproduce that
// check on the frontend: a value from the wrong domain resolves to 未知, the
// same discipline the auth/runtime axes use above. Merging them into one map
// would silently accept a row the backend would have rejected.
// ---------------------------------------------------------------------------

export const EGRESS_DOMAINS = ["egress", "session", "clearance"] as const;
export type EgressDomain = (typeof EGRESS_DOMAINS)[number];

export const EGRESS_STATES = [
  "available",
  "cooling_down",
  "circuit_open",
  "probe_due",
  "probe_in_flight",
  "disabled",
] as const;

export const SESSION_STATES = [
  "absent",
  "active",
  "expired",
  "challenge_required",
  "invalid",
] as const;

export const CLEARANCE_STATES = [
  "absent",
  "fresh",
  "expired",
  "refresh_required",
  "refresh_in_flight",
  "invalid",
] as const;

/** Semantics quoted from the backend enum's own doc comments
 *  (`gateway-control/src/provider_egress_status_service.rs`), not paraphrased
 *  from the state names — several of them are narrower than they look. */
const EGRESS_STATE_META: Readonly<Record<string, StateMeta>> = {
  available: { label: "可用", glyph: "●", tone: "good", detail: "出口可被其所属 channel 选用", en: { label: "Available", detail: "The egress may be selected by its owning channel" },},
  cooling_down: {
    label: "冷却中",
    glyph: "◐",
    tone: "warn",
    detail: "出口冷却至独占截止时刻,到期自动回到可选状态", en: { label: "Cooling", detail: "Cooling until its exclusive deadline, then selectable again" },
  },
  circuit_open: {
    label: "熔断",
    glyph: "▲",
    tone: "serious",
    detail: "出口熔断,直到一次受控探测到期才可能恢复", en: { label: "Circuit open", detail: "Open until a controlled probe becomes due" },
  },
  probe_due: {
    label: "可启动探测",
    glyph: "◇",
    tone: "tint",
    detail: "允许启动一次受控探测 —— 这是许可,不是已经恢复", en: { label: "Probe allowed", detail: "One controlled probe may start — permission, not recovery" },
  },
  probe_in_flight: {
    label: "探测中",
    glyph: "◈",
    tone: "tint",
    detail: "一次受控探测正在进行,结果未知", en: { label: "Probing", detail: "A controlled probe is running; the result is unknown" },
  },
  disabled: { label: "已停用", glyph: "○", tone: "muted", detail: "出口被运维显式停用", en: { label: "Disabled", detail: "The egress is administratively disabled" },},
};

const SESSION_STATE_META: Readonly<Record<string, StateMeta>> = {
  absent: { label: "未建立", glyph: "·", tone: "muted", detail: "会话尚未建立 —— 这是缺席,不是失败", en: { label: "Not established", detail: "No session yet — an absence, not a failure" },},
  active: { label: "有效", glyph: "●", tone: "good", detail: "Provider 会话仍然有效", en: { label: "Active", detail: "The provider session is still valid" },},
  expired: { label: "已过期", glyph: "◐", tone: "serious", detail: "会话已过期", en: { label: "Expired", detail: "The session has expired" },},
  challenge_required: {
    label: "需挑战",
    glyph: "◑",
    tone: "warn",
    detail: "Provider 要求一次显式 challenge", en: { label: "Challenge needed", detail: "The provider requires an explicit challenge" },
  },
  invalid: { label: "血缘无效", glyph: "✕", tone: "critical", detail: "会话血缘无效,不可使用", en: { label: "Invalid lineage", detail: "The session lineage is invalid and cannot be used" },},
};

const CLEARANCE_STATE_META: Readonly<Record<string, StateMeta>> = {
  absent: {
    label: "未建立",
    glyph: "·",
    tone: "muted",
    detail: "放行凭证尚未建立 —— 这是缺席,不是失败", en: { label: "Not established", detail: "No clearance yet — an absence, not a failure" },
  },
  fresh: { label: "新鲜", glyph: "●", tone: "good", detail: "浏览器放行凭证仍然新鲜", en: { label: "Fresh", detail: "The browser clearance is still fresh" },},
  expired: { label: "已过期", glyph: "◐", tone: "serious", detail: "放行凭证已过期", en: { label: "Expired", detail: "The clearance has expired" },},
  refresh_required: {
    label: "需刷新",
    glyph: "◑",
    tone: "warn",
    detail: "放行凭证需要一次显式刷新", en: { label: "Refresh needed", detail: "The clearance needs an explicit refresh" },
  },
  refresh_in_flight: {
    label: "刷新中",
    glyph: "◈",
    tone: "tint",
    detail: "一次有界的放行刷新正在进行", en: { label: "Refreshing", detail: "A bounded clearance refresh is running" },
  },
  invalid: { label: "血缘无效", glyph: "✕", tone: "critical", detail: "放行血缘无效,不可使用", en: { label: "Invalid lineage", detail: "The clearance lineage is invalid and cannot be used" },},
};

const DOMAIN_STATE_META: Readonly<Record<string, Readonly<Record<string, StateMeta>>>> = {
  egress: EGRESS_STATE_META,
  session: SESSION_STATE_META,
  clearance: CLEARANCE_STATE_META,
};

/** A state is only meaningful inside its own domain. Asking for a clearance
 *  state on an egress row is a category error, and it reads as 未知 rather
 *  than resolving through some shared table that would make it look valid. */
export function domainStateMeta(domain: string, state: string): StateMeta {
  return DOMAIN_STATE_META[domain]?.[state] ?? UNKNOWN_META;
}

export const DOMAIN_META: Readonly<Record<EgressDomain, Readonly<{ title: string; help: string }>>> =
  {
    egress: {
      title: "出口 · egress",
      help: "每条 channel 走哪个出口目标,以及那个出口当下能不能被选中。",
    },
    session: {
      title: "会话 · session",
      help: "凭据在该 channel 上的 Provider 会话状态,带凭据与会话两个 revision。",
    },
    clearance: {
      title: "放行 · clearance",
      help: "浏览器放行凭证的新鲜度,额外带出口目标与放行 revision。",
    },
  };

export type EgressStatusRow = Readonly<{
  domain: string;
  provider_id: string;
  upstream_id: string;
  channel_id: string;
  channel_kind: string;
  state: string;
  /** egress + clearance only */
  target_kind?: string;
  target_id?: string | null;
  /** session + clearance only */
  credential_id?: string;
  credential_revision?: number;
  session_revision?: number;
  /** clearance only */
  clearance_revision?: number;
  /** egress only */
  deadline_ms?: number | null;
  /** session + clearance only */
  expires_at_ms?: number | null;
}>;

export type EgressStatusPage = Readonly<{
  config_version_id: string;
  config_revision: number;
  runtime_revision: number;
  snapshot_id: string;
  sampled_at_ms: number;
  items: readonly EgressStatusRow[];
  next_cursor: string | null;
}>;

/** `target_kind` is a closed two-value enum but `target_id` is independently
 *  nullable, so "named with no id reported" is representable and is not the
 *  same thing as direct. Rendering it as a blank cell would erase that. */
export function formatTarget(kind: string | undefined, id: string | null | undefined): string {
  if (kind === undefined) {
    return "—";
  }
  if (kind === "direct") {
    return id === null || id === undefined ? "直连" : `直连 · ${id}`;
  }
  if (kind === "named") {
    return id === null || id === undefined ? "具名出口(未报告 id)" : id;
  }
  return id === null || id === undefined ? kind : `${kind} · ${id}`;
}

/** The two 409s this projection can return mean different things and have
 *  different recoveries, so they are never collapsed into one "conflict":
 *
 *  - `cursor`: the runtime snapshot rotated under an opaque cursor. Nobody
 *    edited anything; re-read this domain from the first page.
 *  - `config`: the selected Config Version is not this snapshot's source.
 *    Re-reading page one with the same version will not help.
 */
export function egressConflictKind(error: unknown): "cursor" | "config" | undefined {
  if (typeof error !== "object" || error === null) {
    return undefined;
  }
  const code = (error as { code?: unknown }).code;
  if (code === "management_provider_egress_status_cursor_conflict") {
    return "cursor";
  }
  if (code === "management_provider_egress_status_config_conflict") {
    return "config";
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// channel pin — POST /admin/operations/channel-pin (P13-08)
//
// This is the only diagnostic on the page that SENDS A REAL REQUEST UPSTREAM.
// It takes If-Match like a mutation, and `attempt_count` is capped at 1, so one
// press is at most one real call against the operator's own quota. Every field
// it accepts is a bounded id or a closed enum — there is deliberately no prompt
// or body input, so the panel cannot be used to send arbitrary content.
// ---------------------------------------------------------------------------

export const PIN_MODES = ["json", "sse"] as const;
export type PinMode = (typeof PIN_MODES)[number];

export const PIN_STAGES = [
  "request_conversion",
  "egress_admission",
  "http_transport",
  "http_status",
  "content_type",
  "body_read",
  "decoder",
  "sse_bootstrap",
] as const;

const PIN_STAGE_LABEL: Readonly<Record<string, string>> = {
  request_conversion: "请求转换",
  egress_admission: "出口准入",
  http_transport: "HTTP 传输",
  http_status: "HTTP 状态码",
  content_type: "内容类型",
  body_read: "读取响应体",
  decoder: "解码",
  sse_bootstrap: "SSE 启动",
};

/** An absent stage is not stage zero — it means no stage was reported. */
export function pinStageLabel(stage: string | undefined): string {
  if (stage === undefined) {
    return "未报告阶段";
  }
  return PIN_STAGE_LABEL[stage] ?? stage;
}

const PIN_OUTCOME_META: Readonly<Record<string, StateMeta>> = {
  succeeded: { label: "成功", glyph: "●", tone: "good", detail: "这条链路走通了", en: { label: "Succeeded", detail: "This path went through" },},
  rejected: {
    label: "被拒绝",
    glyph: "○",
    tone: "muted",
    detail: "网关自己拒绝了这次尝试 —— 是一个答复,不是故障", en: { label: "Declined", detail: "The gateway itself declined the attempt — an answer, not a fault" },
  },
  failed: { label: "失败", glyph: "✕", tone: "serious", detail: "尝试进行了,但没有走通", en: { label: "Failed", detail: "The attempt ran but did not go through" },},
};

export function pinOutcomeMeta(outcome: string): StateMeta {
  return PIN_OUTCOME_META[outcome] ?? UNKNOWN_META;
}

export type PinInput = Readonly<{
  client_key_id?: string;
  credential_revision?: number;
  provider_id: string;
  channel_id: string;
  route_id: string;
  credential_id: string;
  requested_model: string;
  protocol: string;
  mode: string;
}>;

export type PinReceipt = PinInput &
  Readonly<{
    request_id: string;
    config_version_id: string;
    config_revision: number;
    outcome: string;
    upstream_sent: boolean;
    attempt_count: number;
    response_started: boolean;
    observed_at_ms: number;
    stage?: string;
  }>;

/** Every field is required and non-blank; returns undefined rather than sending
 *  a request the contract would refuse. */
export function normalizePinInput(raw: PinInput): PinInput | undefined {
  const trimmed = {
    ...(raw.client_key_id===undefined?{}:{client_key_id:raw.client_key_id.trim()}),
    ...(raw.credential_revision===undefined?{}:{credential_revision:raw.credential_revision}),
    provider_id: raw.provider_id.trim(),
    channel_id: raw.channel_id.trim(),
    route_id: raw.route_id.trim(),
    credential_id: raw.credential_id.trim(),
    requested_model: raw.requested_model.trim(),
    protocol: raw.protocol,
    mode: raw.mode,
  };
  const blank = Object.values(trimmed).some((value) => value === "");
  if (blank || !PROTOCOLS.includes(trimmed.protocol as Protocol)) {
    return undefined;
  }
  return PIN_MODES.includes(trimmed.mode as PinMode) ? trimmed : undefined;
}

/**
 * The four facts a receipt carries that people routinely collapse into one.
 *
 * `upstream_sent: false` with `outcome: "failed"` means the gateway never got
 * as far as the provider — that is a local problem. `upstream_sent: true` with
 * the same outcome means the provider was reached and the call still failed.
 * Reporting either as just "failed" throws away which half to go and look at.
 */
export function pinReach(receipt: PinReceipt): string {
  if (!receipt.upstream_sent) {
    return "请求从未离开网关 —— 问题在上游之前";
  }
  return receipt.response_started
    ? "请求已发出,上游已开始回应"
    : "请求已发出,但上游没有开始回应";
}
