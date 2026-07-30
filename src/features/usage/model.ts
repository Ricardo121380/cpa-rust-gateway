// Usage-analytics pure model (docs/07 §7.2). Everything here is DOM-free and
// deterministic so the URL contract, the per-tab `include` projection and the
// chart scales can be tested without a browser.
//
// Value-free discipline: this module only ever handles closed enums
// (tab / metric / status) and opaque identifiers that the backend already
// returned. It never constructs a request body or a filter value of its own.
import type { AnalyticsFilters, AnalyticsQuery, AnalyticsResponse } from "../../api/proposed-types";

// ---------- tabs ----------

export const USAGE_TABS = ["overview", "trend", "models", "credentials", "heatmap"] as const;
export type UsageTab = (typeof USAGE_TABS)[number];

export const TAB_LABELS: Readonly<Record<UsageTab, string>> = {
  overview: "总览",
  trend: "趋势",
  models: "模型",
  credentials: "凭据",
  heatmap: "热力图",
};

export function parseTab(raw: string | null): UsageTab {
  return USAGE_TABS.find((tab) => tab === raw) ?? "overview";
}

// ---------- metric switch ----------

export const USAGE_METRICS = ["requests", "tokens", "failure_rate"] as const;
export type UsageMetric = (typeof USAGE_METRICS)[number];

export const METRIC_LABELS: Readonly<Record<UsageMetric, string>> = {
  requests: "请求数",
  tokens: "Token",
  failure_rate: "失败率",
};

export function parseMetric(raw: string | null): UsageMetric {
  return USAGE_METRICS.find((metric) => metric === raw) ?? "requests";
}

// ---------- one composite query, projected per tab ----------

/** Only what the visible tab actually renders — plus `options`, which every
 *  tab needs because the filter bar is shared chrome. */
export function includeForTab(tab: UsageTab, metric: UsageMetric): AnalyticsQuery["include"] {
  const base = { options: true } as const;
  switch (tab) {
    case "overview":
      return { ...base, summary: true, timeline: true, ranks: { by: "public_model", limit: 8 } };
    case "trend":
      return { ...base, timeline: true };
    case "models":
      return { ...base, ranks: { by: "public_model", limit: 20 } };
    case "credentials":
      return { ...base, ranks: { by: "credential", limit: 20 } };
    case "heatmap":
      return { ...base, heatmap: { metric } };
  }
}

export type UsageStatus = "all" | "success" | "failed";

export function parseStatus(raw: string | null): UsageStatus {
  return raw === "success" || raw === "failed" ? raw : "all";
}

export function buildFilters(status: UsageStatus, model: string | null): AnalyticsFilters {
  return {
    status,
    ...(model !== null && model !== "" ? { public_model: [model] } : {}),
  };
}

export function hasActiveFilter(status: UsageStatus, model: string | null): boolean {
  return status !== "all" || (model !== null && model !== "");
}

// ---------- timeline metric extraction ----------

export type TimelineBucket = NonNullable<AnalyticsResponse["timeline"]>[number];

export function metricValue(bucket: TimelineBucket, metric: UsageMetric): number {
  switch (metric) {
    case "requests":
      return bucket.requests;
    case "tokens":
      return bucket.tokens_total;
    case "failure_rate":
      return bucket.requests > 0 ? bucket.failures / bucket.requests : 0;
  }
}

export function formatMetric(value: number, metric: UsageMetric): string {
  if (metric === "failure_rate") {
    return `${(value * 100).toFixed(2)}%`;
  }
  return formatAxisNumber(value);
}

/** Axis / tooltip number: grouped, never scientific, never a fake precision. */
export function formatAxisNumber(value: number): string {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 10_000) return `${(value / 1_000).toFixed(1)}K`;
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

// ---------- scales ----------
// The axis maths lives with the chart primitives; re-exported here so the page
// and its tests have one import surface.
export { axisTicks, niceCeil } from "../../components/data/scale";

// ---------- heatmap ----------

export const HEAT_STEPS = 6;

/** 0 = no traffic, 1..HEAT_STEPS-1 = one lightness step of a single hue.
 *  A zero cell is never given a colour step: "no data" must not read as "low". */
export function heatStep(value: number, max: number, steps = HEAT_STEPS): number {
  if (value <= 0 || max <= 0) return 0;
  const ratio = value / max;
  return Math.min(steps - 1, 1 + Math.floor(ratio * (steps - 1) * 0.999999));
}

/** Upper bound of each coloured bin, for the legend. */
export function heatBins(max: number, steps = HEAT_STEPS): number[] {
  const top = max > 0 ? max : 1;
  return Array.from({ length: steps - 1 }, (_, index) => (top / (steps - 1)) * (index + 1));
}

// weekday 0 = 周日, matching Date#getDay(). The G3 proposal does not pin the
// convention; anchoring it to getDay() is what keeps the cell → time-window
// deep link honest, because the same call does the inverse mapping.
export const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

export type HeatSelection = Readonly<{ weekday: number; hour: number }>;

/** `?cell=w-h` — the selected heatmap cell is part of the URL contract, so a
 *  revealed detail panel survives a reload and can be linked to. */
export function parseCell(raw: string | null): HeatSelection | null {
  if (raw === null) return null;
  const matched = /^([0-6])-(\d{1,2})$/u.exec(raw);
  if (matched === null) return null;
  const hour = Number(matched[2]);
  if (hour > 23) return null;
  return { weekday: Number(matched[1]), hour };
}

export function cellParam(cell: HeatSelection): string {
  return `${cell.weekday}-${cell.hour}`;
}

export type CellWindow = Readonly<{ from_ms: number; to_ms: number }>;

const HOUR_MS = 3_600_000;

/** The most recent [hour, hour+1) inside the range whose LOCAL weekday/hour
 *  match the clicked cell. `null` when the range does not cover that cell —
 *  in which case there is nothing honest to deep-link to. */
export function cellWindow(
  weekday: number,
  hour: number,
  from_ms: number,
  to_ms: number,
): CellWindow | null {
  const cursor = new Date(to_ms);
  cursor.setMinutes(0, 0, 0);
  for (let step = 0; step < 24 * 7 + 24; step += 1) {
    const start = cursor.getTime();
    if (start < from_ms) break;
    if (cursor.getDay() === weekday && cursor.getHours() === hour) {
      return { from_ms: start, to_ms: Math.min(start + HOUR_MS, to_ms) };
    }
    cursor.setTime(start - HOUR_MS);
  }
  return null;
}

/** Deep link into 请求监控 with the window and the live filters encoded — the
 *  target page parses them back out of the URL (docs/07 §6 深链下钻). */
export function monitoringHref(
  window: CellWindow,
  status: UsageStatus,
  model: string | null,
): string {
  const params = new URLSearchParams({
    range: "custom",
    from: String(window.from_ms),
    to: String(window.to_ms),
    bucket: "hour",
  });
  if (status !== "all") params.set("status", status);
  if (model !== null && model !== "") params.set("model", model);
  return `/monitoring?${params.toString()}`;
}

// ---------- rank tables ----------

export type RankRow = NonNullable<AnalyticsResponse["ranks"]>[number];

export function shareOf(value: number, total: number): number {
  return total > 0 ? value / total : 0;
}

export function failureRate(row: RankRow): number {
  return row.requests > 0 ? row.failures / row.requests : 0;
}

export function rankTotal(rows: readonly RankRow[]): number {
  return rows.reduce((sum, row) => sum + row.requests, 0);
}
