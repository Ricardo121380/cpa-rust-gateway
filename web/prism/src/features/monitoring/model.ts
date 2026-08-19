// 请求监控 — pure model over two INDEPENDENT contract sources.
//
// This replaces a model written for the PROPOSED G3 analytics shape, whose
// request stream carried latency, an outcome enum and a per-request error. None
// of those exist in the delivered contract, so the page they fed could not be
// rewired — it had to be redesigned. What actually exists:
//
//   GET /admin/operations/billing
//     One row per BILLED request: ids, six token counts, cost, and a cost
//     confidence. No latency, no outcome, no error. NOT version-scoped.
//
//   GET /admin/operations/provider-account-pools/failures
//     One row per ATTRIBUTED FAILED ATTEMPT: error code / scope / retry
//     decision. No latency, no cost, no tokens. IS version-scoped.
//
//   GET /admin/requests/{request_id}/attempts
//     The per-attempt trail for one request. A bare array — no cursor, no
//     paging, no time filter. NOT version-scoped.
//
// Three consequences the UI must carry rather than paper over:
//
//   1. THERE IS NO LATENCY ANYWHERE. No P50/P95 is derivable. The old page
//      showed both.
//   2. THE TWO STREAMS ARE NOT TWO HALVES OF ONE TOTAL. The ledger holds
//      requests that produced a usage record; the failure stream holds attempts
//      attributed to an account. A request can appear in both, in neither, or
//      many times in the second. A "success rate" computed from them would be
//      a fabrication.
//   3. THEY DISAGREE ON SCOPE. Failures require X-Config-Version; the ledger
//      forbids it. Selecting a version in the top bar changes ONE panel.
//
// Nothing here touches the DOM, the clock or the network.

export type Tone = "good" | "warn" | "serious" | "critical" | "tint" | "muted";

// ---------------------------------------------------------------------------
// billing ledger
// ---------------------------------------------------------------------------

/** The contract's `status` query parameter selects COST CONFIDENCE, not request
 *  outcome. The name invites exactly the wrong reading, so the UI never labels
 *  this control "状态". */
export const COST_CONFIDENCES = ["exact", "partial", "unknown", "unpriced"] as const;
export type CostConfidence = (typeof COST_CONFIDENCES)[number];

const COST_CONFIDENCE_META: Readonly<Record<CostConfidence, { label: string; tone: Tone; detail: string }>> = {
  exact: { label: "精确", tone: "good", detail: "目录覆盖了这条记录的全部计价维度" },
  partial: { label: "部分", tone: "warn", detail: "部分维度有价,成本是不完整的" },
  unknown: { label: "未知", tone: "muted", detail: "token 计数缺失,无法据此计价" },
  unpriced: { label: "无价格", tone: "serious", detail: "绑定目录里没有该模型的费率 —— 不是零成本" },
};

export function costConfidenceLabel(value: string): string {
  return COST_CONFIDENCE_META[value as CostConfidence]?.label ?? value;
}

export function costConfidenceTone(value: string): Tone {
  return COST_CONFIDENCE_META[value as CostConfidence]?.tone ?? "muted";
}

export function costConfidenceDetail(value: string): string {
  return COST_CONFIDENCE_META[value as CostConfidence]?.detail ?? value;
}

export type LedgerRow = Readonly<{
  ledger_id: number;
  request_id: string;
  response_id: string;
  provider_id: string;
  channel_id: string;
  account_id: string;
  model: string;
  input_tokens: number | null;
  output_tokens: number | null;
  reasoning_tokens: number | null;
  cache_read_tokens: number | null;
  cache_creation_tokens: number | null;
  cached_tokens: number | null;
  occurred_at_ms: number;
  catalog_version_id: string | null;
  cost_microunits: number | null;
  cost_confidence: CostConfidence;
}>;

export type BillingSummary = Readonly<{
  records: number;
  exact_records: number;
  partial_records: number;
  unknown_records: number;
  unpriced_records: number;
  known_cost_microunits: number | null;
}>;

export type BillingResponse = Readonly<{
  snapshot_ledger_id: number | null;
  items: readonly LedgerRow[];
  summary: BillingSummary;
  next_cursor: string | null;
}>;

/** Share of records whose cost is exactly known. This is the honest headline
 *  for a billing page: not "how much did it cost" but "how much of it do we
 *  actually know". Returns null when there is nothing to divide. */
export function exactShare(summary: BillingSummary): number | null {
  return summary.records <= 0 ? null : summary.exact_records / summary.records;
}

/** True when the summary's own counts do not add up to its record total.
 *  The four confidence buckets are declared as a partition; if a deployment
 *  ever breaks that, showing the parts as if they summed would hide it. */
export function summaryIsPartitioned(summary: BillingSummary): boolean {
  return (
    summary.exact_records +
      summary.partial_records +
      summary.unknown_records +
      summary.unpriced_records ===
    summary.records
  );
}

// ---------------------------------------------------------------------------
// failure attribution
// ---------------------------------------------------------------------------

/** The seventeen closed error codes. Labels are for scanning; an unknown member
 *  renders as itself rather than being coerced into a neighbour. */
const ERROR_CODE_LABEL: Readonly<Record<string, string>> = {
  ClientRequestError: "客户端请求有误",
  ClientUnauthorized: "客户端未授权",
  RouteNotFound: "找不到路由",
  CredentialUnavailable: "无可用凭据",
  CredentialUnauthorized: "凭据未授权",
  CredentialForbidden: "凭据被上游拒绝",
  CredentialQuotaExceeded: "凭据配额耗尽",
  EgressRejected: "出口策略拒绝",
  EgressUnavailable: "出口不可用",
  ProviderRateLimited: "上游限流",
  ProviderTransient: "上游瞬时故障",
  ProviderPermanent: "上游永久错误",
  TokenCountUnsupported: "上游不支持 token 计数",
  UpstreamProtocolError: "上游协议错误",
  StreamTruncated: "流被截断",
  InternalError: "网关内部错误",
  Cancelled: "已取消",
};

export function errorCodeLabel(code: string): string | undefined {
  return ERROR_CODE_LABEL[code];
}

/** Which layer the failure is attributed to. This is a classification, not a
 *  severity — it says where to look, not how bad it is. */
const ERROR_SCOPE_LABEL: Readonly<Record<string, string>> = {
  request: "请求",
  credential: "凭据",
  account: "账号",
  model: "模型",
  quota_window: "配额窗",
  egress_session: "出口会话",
  egress: "出口",
  provider: "上游",
  stream: "流",
  internal: "内部",
};

export function errorScopeLabel(scope: string): string {
  return ERROR_SCOPE_LABEL[scope] ?? scope;
}

/** What the scheduler did next. This IS a severity axis: `retry_eligible` means
 *  the request may still have succeeded on another attempt, while
 *  `non_retryable` means it did not. */
const RETRY_META: Readonly<Record<string, { label: string; tone: Tone; detail: string }>> = {
  completed: { label: "已完成", tone: "good", detail: "该尝试之后请求走完了流程" },
  retry_eligible: { label: "可重试", tone: "warn", detail: "允许再试 —— 请求可能仍然成功了" },
  non_retryable: { label: "不可重试", tone: "critical", detail: "失败终止,不再尝试" },
  retry_closed: { label: "重试已关闭", tone: "serious", detail: "重试预算或首字节边界已用尽" },
  cancelled: { label: "已取消", tone: "muted", detail: "客户端或网关取消" },
  infrastructure_failure: { label: "基础设施故障", tone: "critical", detail: "网关自身或依赖出错" },
};

export function retryLabel(decision: string): string {
  return RETRY_META[decision]?.label ?? decision;
}

export function retryTone(decision: string): Tone {
  return RETRY_META[decision]?.tone ?? "muted";
}

export function retryDetail(decision: string): string {
  return RETRY_META[decision]?.detail ?? decision;
}

export type FailureRow = Readonly<{
  provider_id: string;
  channel_id: string;
  account_id: string;
  request_id: string;
  attempt_id: string;
  ended_at_ms: number;
  error_code: string;
  error_scope: string;
  retry_decision: string;
}>;

export type FailureResponse = Readonly<{
  observed_through_ordinal: number | null;
  items: readonly FailureRow[];
  next_cursor: string | null;
}>;

/** Counts by a closed field, descending. Used for the failure breakdown, which
 *  is a count over the rows ACTUALLY FETCHED — never extrapolated to the whole
 *  stream, because the cursor may not have been followed to the end. */
export function tally(
  rows: readonly FailureRow[],
  field: "error_code" | "error_scope" | "retry_decision",
): ReadonlyArray<Readonly<{ key: string; count: number }>> {
  const counts = new Map<string, number>();
  for (const row of rows) {
    counts.set(row[field], (counts.get(row[field]) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key));
}

// ---------------------------------------------------------------------------
// per-request attempts (drill-down)
// ---------------------------------------------------------------------------

/** `outcome` is a plain string in the contract, NOT an enum — so it is rendered
 *  verbatim. `stage` is the closed eight-member set. */
export type AttemptRow = Readonly<{
  attempt_id: string;
  outcome: string;
  stage?: string | null;
  endpoint_id?: string | null;
  credential_id?: string | null;
}>;

const STAGE_LABEL: Readonly<Record<string, string>> = {
  request_conversion: "请求转换",
  egress_admission: "出口准入",
  http_transport: "HTTP 传输",
  http_status: "HTTP 状态",
  content_type: "内容类型",
  body_read: "读取响应体",
  decoder: "解码",
  sse_bootstrap: "SSE 建流",
};

export function stageLabel(stage: string): string {
  return STAGE_LABEL[stage] ?? stage;
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

/**
 * Cost is in MICROUNITS and the contract names no currency anywhere — not in
 * the ledger, not in the catalog (`input_microunits_per_million`). So this
 * never renders a currency symbol and never divides into "dollars": doing
 * either would assert a unit the backend has not stated.
 */
export function formatMicrounits(value: number | null): string {
  return value === null ? "—" : value.toLocaleString("en-US");
}

export function formatTokens(value: number | null): string {
  return value === null ? "—" : value.toLocaleString("en-US");
}

export function formatCount(value: number): string {
  return value.toLocaleString("en-US");
}

/** UTC, second precision — the wall clock the gateway logs use. */
export function formatTime(ms: number): string {
  return `${new Date(ms).toISOString().slice(0, 19).replace("T", " ")}Z`;
}

export function formatPercent(share: number | null): string {
  return share === null ? "—" : `${(share * 100).toFixed(1)}%`;
}

// ---------------------------------------------------------------------------
// tabs, filters, paging
// ---------------------------------------------------------------------------

export const TABS = ["ledger", "failures"] as const;
export type Tab = (typeof TABS)[number];

export function parseTab(raw: string | null): Tab {
  return TABS.find((tab) => tab === raw) ?? "ledger";
}

export const LEDGER_FILTER_KEYS = [
  "provider_id",
  "channel_id",
  "account_id",
  "model",
  "status",
] as const;

export const FAILURE_FILTER_KEYS = ["provider_id", "channel_id", "account_id"] as const;

export type FilterKey =
  | (typeof LEDGER_FILTER_KEYS)[number]
  | (typeof FAILURE_FILTER_KEYS)[number];

const FILTER_LABEL: Readonly<Record<FilterKey, string>> = {
  provider_id: "Provider",
  channel_id: "Channel",
  account_id: "账号",
  model: "模型",
  // NOT "状态": this selects cost confidence. See COST_CONFIDENCES.
  status: "计价置信度",
};

export function filterLabel(key: FilterKey): string {
  return FILTER_LABEL[key];
}

export function parseFilters(
  keys: readonly FilterKey[],
  get: (key: string) => string | null,
): Readonly<Record<string, string>> {
  const filters: Record<string, string> = {};
  for (const key of keys) {
    const raw = get(key)?.trim() ?? "";
    if (raw.length === 0) {
      continue;
    }
    if (key === "status" && !COST_CONFIDENCES.some((value) => value === raw)) {
      continue; // closed enum: an unknown value is dropped, never forwarded
    }
    filters[key] = raw;
  }
  return filters;
}

/** Contract cap. */
export const PAGE_LIMIT = 100;

/**
 * Both streams are unbounded in principle and neither view needs a grand total
 * of its own, so unlike 用量分析 this page pages EXPLICITLY (a "load more"
 * button) rather than fetching to completion.
 *
 * That is safe for the ledger because its `summary` is not a page summary.
 * Verified in the backend
 * (gateway-control/src/management_operations_service.rs): the accumulation loop
 * runs over the fully filtered set, pinned to `snapshot_ledger_id`, and only
 * afterwards does the cursor trim and `truncate(limit)` apply. So the
 * credibility figures are correct from page one.
 *
 * The failure breakdown has no such summary, so its counts are explicitly
 * labelled as covering the rows loaded so far.
 */
export const MAX_PAGES = 20;
