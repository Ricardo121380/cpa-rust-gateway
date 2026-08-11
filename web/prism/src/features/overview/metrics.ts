// Shapes the gateway's bounded Prometheus counters into what the overview can
// honestly show.
//
// What this plane IS: process-lifetime cumulative counters, reset on restart.
// What it is NOT: G3 analytics. There is no time bucket, no per-model or
// per-key dimension, no latency quantile — the renderer deliberately adds "no
// request-scoped or target-scoped label". Anything that needs a time window
// stays behind the G3 gap; do not synthesise it from these.
import { parsePrometheus, pick } from "../../api/prometheus";
import type { TokenSummary } from "../../api/proposed-types";

const P = "gateway_observability_";

const EVENT_KINDS = ["request", "attempt", "usage", "health", "diagnostic"] as const;
export type EventKind = (typeof EVENT_KINDS)[number];

// Keys match TokenSummary exactly, which is also the metric's `kind` label set.
const TOKEN_KINDS = [
  "input",
  "output",
  "reasoning",
  "cache_read",
  "cache_creation",
  "cached",
] as const;

// Every counter that means "an event the pipeline handled did not survive".
//
// Severity is not cosmetic: the queue separates Required from Diagnostic
// precisely so diagnostics can be shed under pressure. A non-zero
// `diagnostic_dropped` is the design working; a non-zero Required counter is
// data the durable log was supposed to keep and lost. Rendering both at the
// same volume would train the operator to ignore the one that matters.
const LOSS_SOURCES = [
  { metric: `${P}queue_admission_total`, outcome: "required_queue_full", severity: "required", label: "必需事件被队列拒绝" },
  { metric: `${P}durable_events_total`, outcome: "required_quarantined", severity: "required", label: "必需事件被隔离" },
  { metric: `${P}durable_events_total`, outcome: "write_failed", severity: "required", label: "持久化写失败" },
  { metric: `${P}queue_admission_total`, outcome: "sink_closed", severity: "required", label: "接收端已关闭" },
  { metric: `${P}queue_admission_total`, outcome: "diagnostic_dropped", severity: "diagnostic", label: "诊断事件被丢弃" },
] as const;

export type LossSeverity = (typeof LOSS_SOURCES)[number]["severity"];
export type LossSignal = Readonly<{
  key: string;
  label: string;
  value: number;
  severity: LossSeverity;
}>;

export type GatewayCounters = Readonly<{
  events: Readonly<Record<EventKind, number>>;
  eventsTotal: number;
  attempts: Readonly<{ succeeded: number; failed: number; total: number }>;
  tokens: TokenSummary;
  /** Only the non-zero loss counters, Required before Diagnostic. Empty means nothing dropped. */
  loss: readonly LossSignal[];
  /** Events the durable log was meant to keep and did not. Non-zero is an alarm. */
  requiredLoss: number;
  /** Diagnostics shed under pressure. Non-zero is the backpressure design working. */
  diagnosticLoss: number;
  /** Required events still sitting in the writer's one bounded pending batch. */
  pendingRequired: number;
}>;

export function readCounters(exposition: string): GatewayCounters {
  const samples = parsePrometheus(exposition);
  const events = Object.fromEntries(
    EVENT_KINDS.map((kind) => [kind, pick(samples, `${P}events_total`, { kind })]),
  ) as Record<EventKind, number>;
  const tokens = Object.fromEntries(
    TOKEN_KINDS.map((kind) => [kind, pick(samples, `${P}usage_tokens_total`, { kind })]),
  ) as Record<(typeof TOKEN_KINDS)[number], number>;

  const succeeded = pick(samples, `${P}attempts_total`, { outcome: "succeeded" });
  const failed = pick(samples, `${P}attempts_total`, { outcome: "failed" });

  const loss = LOSS_SOURCES.map((source) => ({
    key: source.outcome,
    label: source.label,
    severity: source.severity,
    value: pick(samples, source.metric, { outcome: source.outcome }),
  })).filter((signal) => signal.value > 0);

  const bySeverity = (severity: LossSeverity) =>
    loss.filter((signal) => signal.severity === severity).reduce((sum, s) => sum + s.value, 0);

  return {
    events,
    eventsTotal: EVENT_KINDS.reduce((sum, kind) => sum + events[kind], 0),
    attempts: { succeeded, failed, total: succeeded + failed },
    tokens,
    loss,
    requiredLoss: bySeverity("required"),
    diagnosticLoss: bySeverity("diagnostic"),
    pendingRequired: pick(samples, `${P}durable_pending_required`),
  };
}

/** Attempt success ratio, or undefined while no attempt has been observed. */
export function successRate(counters: GatewayCounters): number | undefined {
  return counters.attempts.total === 0
    ? undefined
    : counters.attempts.succeeded / counters.attempts.total;
}

/**
 * Growth since the first scrape of this page visit. Counters are cumulative
 * over the gateway process, so the lifetime number says little about now —
 * the delta is what the operator is actually watching.
 */
export function growthSince(
  first: GatewayCounters | undefined,
  latest: GatewayCounters,
): Readonly<{ attempts: number; failed: number; events: number }> | undefined {
  if (first === undefined) {
    return undefined;
  }
  // A restart resets the counters; a lower value means the baseline is gone.
  if (latest.attempts.total < first.attempts.total || latest.eventsTotal < first.eventsTotal) {
    return undefined;
  }
  return {
    attempts: latest.attempts.total - first.attempts.total,
    failed: latest.attempts.failed - first.attempts.failed,
    events: latest.eventsTotal - first.eventsTotal,
  };
}
