// 用量分析 — pure model over GET /admin/operations/usage (P13-04B).
//
// This module replaces one written for the PROPOSED G3 analytics shape, which
// the backend never implemented. That version had tabs, time buckets, a heatmap
// and a zoom brush; in production it rendered a "contract pending" empty state
// and nothing else, because its data source resolved to `false` outside dev
// fixtures. What the real contract returns is a different thing entirely:
//
//   * NO SERVER-SIDE TIME BUCKETS. One row is an aggregate over the whole
//     [from_ms, to_ms] window for one exact 7-tuple (provider, channel,
//     account, public_model, protocol, client_key, access_group).
//     `observed_at_ms` is a watermark on the row, not a bucket stamp. There is
//     no way to ask for a series — so there is no trend line, no heatmap and no
//     dataZoom here, and adding one would mean inventing data.
//   * `limit` MAXES OUT AT 100. Any real deployment pages, so a total computed
//     from one page is simply wrong — see `collect` and `truncated`.
//   * SIX TOKEN FAMILIES, EACH WITH ITS OWN CONFIDENCE, and `total` is nullable.
//     `null` means "not observed", which is NOT zero. Summing it as zero is the
//     easiest way to under-report and look precise while doing it.
//   * `cost_confidence` is the one-member enum "unpriced". Cost belongs to
//     /admin/operations/billing; this page shows tokens, never money.
//
// Nothing here touches the DOM, the clock or the network.

/** Shared with StatusBadge / StateChip (docs/07 §8.6). */
export type Tone = "good" | "warn" | "serious" | "critical" | "tint" | "muted";

export const PROTOCOLS = [
  "openai_chat_completions",
  "openai_responses",
  "anthropic_messages",
] as const;
export type Protocol = (typeof PROTOCOLS)[number];

/** Per-family observation quality. */
export const CONFIDENCES = ["exact", "partial", "unknown"] as const;
export type Confidence = (typeof CONFIDENCES)[number];

export type TokenFamily = Readonly<{
  total: number | null;
  confidence: Confidence;
}>;

/** The six families, in the order the contract declares them. */
export const TOKEN_FAMILIES = [
  "input_tokens",
  "output_tokens",
  "reasoning_tokens",
  "cache_read_tokens",
  "cache_creation_tokens",
  "cached_tokens",
] as const;

export type TokenFamilyName = (typeof TOKEN_FAMILIES)[number];

export type UsageRow = Readonly<{
  provider_id: string;
  channel_id: string;
  account_id: string;
  public_model: string;
  protocol: Protocol;
  client_key_id: string;
  access_group_id: string | null;
  request_count: number;
  usage_observations: number;
  observed_at_ms: number;
  cost_microunits: number | null;
  cost_confidence: "unpriced";
}> &
  Readonly<Record<TokenFamilyName, TokenFamily>>;

export type UsageResponse = Readonly<{
  observed_through_ms: number | null;
  items: readonly UsageRow[];
  next_cursor: string | null;
}>;

// ---------------------------------------------------------------------------
// grouping dimensions
// ---------------------------------------------------------------------------

/** The seven row dimensions, each usable as a grouping key. They are exactly
 *  the fields the contract also accepts as filters. Nothing derived. */
export const DIMENSIONS = [
  "provider_id",
  "channel_id",
  "account_id",
  "public_model",
  "protocol",
  "client_key_id",
  "access_group_id",
] as const;

export type Dimension = (typeof DIMENSIONS)[number];

const DIMENSION_LABEL: Readonly<Record<Dimension, string>> = {
  provider_id: "Provider",
  channel_id: "Channel",
  account_id: "账号",
  public_model: "公开模型",
  protocol: "协议",
  client_key_id: "Client Key",
  access_group_id: "访问组",
};

export function dimensionLabel(dimension: Dimension): string {
  return DIMENSION_LABEL[dimension];
}

export function parseDimension(raw: string | null): Dimension {
  return DIMENSIONS.find((dimension) => dimension === raw) ?? "provider_id";
}

/** access_group_id is the one nullable dimension: a Client Key need not belong
 *  to a group. `null` becomes its own bucket, never folded into "" or dropped —
 *  "no access group" is a real answer about the deployment. */
export const UNGROUPED_LABEL = "(无访问组)";

export function dimensionValue(row: UsageRow, dimension: Dimension): string | null {
  return row[dimension];
}

// ---------------------------------------------------------------------------
// confidence arithmetic — the part that must not lie
// ---------------------------------------------------------------------------

const RANK: Readonly<Record<Confidence, number>> = { exact: 0, partial: 1, unknown: 2 };

/** A sum is only as good as its worst contributor. */
export function weakest(values: readonly Confidence[]): Confidence {
  let worst: Confidence = "exact";
  for (const value of values) {
    if (RANK[value] > RANK[worst]) {
      worst = value;
    }
  }
  return worst;
}

export type FamilyTotal = Readonly<{
  /** sum of contributors that reported a number; null when none did */
  total: number | null;
  confidence: Confidence;
  /** true when at least one contributor reported `total: null` — the number
   *  above is then a LOWER BOUND, not a total, and the UI must say so. */
  partialCoverage: boolean;
}>;

/**
 * Adds one token family across rows.
 *
 * `total: null` contributors are NOT counted as zero — they are unobserved.
 * They force the confidence to "unknown" and raise `partialCoverage`, because a
 * sum missing an unknown quantity is a floor. Printing it as a total would be
 * precise and wrong at the same time.
 */
export function sumFamily(rows: readonly UsageRow[], family: TokenFamilyName): FamilyTotal {
  let total: number | null = null;
  let partialCoverage = false;
  const confidences: Confidence[] = [];
  for (const row of rows) {
    const value = row[family];
    confidences.push(value.confidence);
    if (value.total === null) {
      partialCoverage = true;
      continue;
    }
    total = (total ?? 0) + value.total;
  }
  return {
    total,
    confidence: partialCoverage ? "unknown" : weakest(confidences),
    partialCoverage,
  };
}

export type Group = Readonly<{
  key: string;
  /** null only for access_group_id */
  value: string | null;
  rows: readonly UsageRow[];
  request_count: number;
  usage_observations: number;
  families: Readonly<Record<TokenFamilyName, FamilyTotal>>;
}>;

/** Groups rows by one dimension and sums each family within the group. Sorted
 *  by request_count descending then key, so identical data always renders in
 *  the same order. */
export function groupBy(rows: readonly UsageRow[], dimension: Dimension): readonly Group[] {
  const buckets = new Map<string, { value: string | null; rows: UsageRow[] }>();
  for (const row of rows) {
    const value = dimensionValue(row, dimension);
    const key = value ?? UNGROUPED_LABEL;
    const bucket = buckets.get(key) ?? { value, rows: [] };
    bucket.rows.push(row);
    buckets.set(key, bucket);
  }
  const groups = [...buckets.entries()].map(([key, bucket]) => ({
    key,
    value: bucket.value,
    rows: bucket.rows as readonly UsageRow[],
    request_count: bucket.rows.reduce((sum, row) => sum + row.request_count, 0),
    usage_observations: bucket.rows.reduce((sum, row) => sum + row.usage_observations, 0),
    families: Object.fromEntries(
      TOKEN_FAMILIES.map((family) => [family, sumFamily(bucket.rows, family)]),
    ) as Record<TokenFamilyName, FamilyTotal>,
  }));
  groups.sort((a, b) => b.request_count - a.request_count || a.key.localeCompare(b.key));
  return groups;
}

/** Share of the request total, for the inline bar. Guards the empty case so a
 *  zero-row page cannot produce NaN widths. */
export function shareOf(value: number, total: number): number {
  return total <= 0 ? 0 : value / total;
}

// ---------------------------------------------------------------------------
// filters — only closed enums and ids the backend already returned
// ---------------------------------------------------------------------------

export const FILTER_KEYS = [
  "provider_id",
  "channel_id",
  "account_id",
  "model",
  "client_key_id",
  "access_group_id",
  "protocol",
] as const;

export type FilterKey = (typeof FILTER_KEYS)[number];
export type Filters = Partial<Readonly<Record<FilterKey, string>>>;

const FILTER_LABEL: Readonly<Record<FilterKey, string>> = {
  provider_id: "Provider",
  channel_id: "Channel",
  account_id: "账号",
  model: "公开模型",
  client_key_id: "Client Key",
  access_group_id: "访问组",
  protocol: "协议",
};

export function filterLabel(key: FilterKey): string {
  return FILTER_LABEL[key];
}

/** Reads filters out of the URL. An unknown protocol is DROPPED rather than
 *  forwarded: the contract declares a closed enum, and passing anything else
 *  earns a 400 that reads as a panel bug. */
export function parseFilters(get: (key: string) => string | null): Filters {
  const filters: Record<string, string> = {};
  for (const key of FILTER_KEYS) {
    const raw = get(key)?.trim() ?? "";
    if (raw.length === 0) {
      continue;
    }
    if (key === "protocol" && !PROTOCOLS.some((protocol) => protocol === raw)) {
      continue;
    }
    filters[key] = raw;
  }
  return filters;
}

export function activeFilterCount(filters: Filters): number {
  return Object.values(filters).filter((value) => value !== undefined && value.length > 0).length;
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

const CONFIDENCE_LABEL: Readonly<Record<Confidence, string>> = {
  exact: "精确",
  partial: "部分",
  unknown: "未知",
};

const CONFIDENCE_TONE: Readonly<Record<Confidence, Tone>> = {
  exact: "good",
  partial: "warn",
  unknown: "muted",
};

export function confidenceLabel(confidence: Confidence): string {
  return CONFIDENCE_LABEL[confidence];
}

export function confidenceTone(confidence: Confidence): Tone {
  return CONFIDENCE_TONE[confidence];
}

const FAMILY_LABEL: Readonly<Record<TokenFamilyName, string>> = {
  input_tokens: "输入",
  output_tokens: "输出",
  reasoning_tokens: "推理",
  cache_read_tokens: "缓存读",
  cache_creation_tokens: "缓存写",
  cached_tokens: "已缓存",
};

export function familyLabel(family: TokenFamilyName): string {
  return FAMILY_LABEL[family];
}

/** `null` renders as an em dash, never as 0: "not observed" and "zero tokens"
 *  are different facts and this page must not merge them. */
export function formatTokens(total: number | null): string {
  return total === null ? "—" : total.toLocaleString("en-US");
}

export function formatCount(value: number): string {
  return value.toLocaleString("en-US");
}

/** UTC, minute precision — the same wall clock the gateway logs use. */
export function formatWatermark(observedThroughMs: number | null): string {
  if (observedThroughMs === null) {
    return "尚无观测";
  }
  return `${new Date(observedThroughMs).toISOString().slice(0, 16).replace("T", " ")}Z`;
}

// ---------------------------------------------------------------------------
// time range
// ---------------------------------------------------------------------------

export const RANGE_PRESETS = ["24h", "7d", "30d", "all"] as const;
export type RangePreset = (typeof RANGE_PRESETS)[number];

const RANGE_LABEL: Readonly<Record<RangePreset, string>> = {
  "24h": "24 小时",
  "7d": "7 天",
  "30d": "30 天",
  all: "全部",
};

export function rangeLabel(preset: RangePreset): string {
  return RANGE_LABEL[preset];
}

export function parseRange(raw: string | null): RangePreset {
  return RANGE_PRESETS.find((preset) => preset === raw) ?? "7d";
}

const RANGE_MS: Readonly<Record<Exclude<RangePreset, "all">, number>> = {
  "24h": 86_400_000,
  "7d": 604_800_000,
  "30d": 2_592_000_000,
};

/** "all" omits both bounds rather than sending `from_ms: 0` — an explicit zero
 *  is a filter the backend must honour, while omission lets it answer over its
 *  own retention window. */
export function rangeParams(
  preset: RangePreset,
  nowMs: number,
): Readonly<{ from_ms?: number; to_ms?: number }> {
  if (preset === "all") {
    return {};
  }
  return { from_ms: nowMs - RANGE_MS[preset], to_ms: nowMs };
}

// ---------------------------------------------------------------------------
// paging
// ---------------------------------------------------------------------------

/** The contract caps `limit` at 100, so this is the largest legal page. */
export const PAGE_LIMIT = 100;

/** Hard stop on cursor-following. Bounded so a fetch-all cannot run away on a
 *  deployment with a very wide 7-tuple space; when it trips the page SAYS SO
 *  rather than presenting a truncated sum as a total. */
export const MAX_PAGES = 20;

export type Collected = Readonly<{
  rows: readonly UsageRow[];
  observed_through_ms: number | null;
  /** true when MAX_PAGES was reached with a cursor still outstanding */
  truncated: boolean;
  pages: number;
}>;

/**
 * Folds successive pages into one collection.
 *
 * Totals are computed over ALL fetched rows, so this must follow the cursor:
 * summing the first page alone would under-report whenever the 7-tuple space
 * exceeds 100 entries, which is the normal case for anything but a toy config.
 */
export function collect(pages: readonly UsageResponse[]): Collected {
  const last = pages.at(-1);
  return {
    rows: pages.flatMap((page) => [...page.items]),
    // Every page carries the same watermark; the last read is the freshest.
    observed_through_ms: last?.observed_through_ms ?? null,
    truncated: pages.length >= MAX_PAGES && (last?.next_cursor ?? null) !== null,
    pages: pages.length,
  };
}
