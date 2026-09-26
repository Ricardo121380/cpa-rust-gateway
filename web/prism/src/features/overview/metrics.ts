// Shapes the gateway's bounded Prometheus counters into what the overview can
// honestly show.
//
// What this plane IS: process-lifetime cumulative counters, reset on restart.
// What it is NOT: request analytics. There is no time bucket, no per-model or
// per-key dimension, no latency quantile in this process-lifetime plane.
// Request-terminal summaries provide those in a separate snapshot; never
// synthesise them from these counters.
import { parsePrometheus, pick } from "../../api/prometheus";
/** The token families the Prometheus exposition carries. Lived in
 *  api/proposed-types until that module went with the analytics shape it
 *  described; it is a property of these counters, not of any endpoint. */
export type TokenSummary = Readonly<{
  input?: number;
  output?: number;
  reasoning?: number;
  cache_read?: number;
  cache_creation?: number;
  cached?: number;
}>;

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
  /** Only the non-zero loss counters, Required before Diagnostic. Empty means no alarms in these process counters; not proof of complete history. */
  loss: readonly LossSignal[];
  /** Sum of recording alarms, which can describe retries of the same event. Not a count of lost records. */
  requiredLoss: number;
  /** Diagnostics shed under pressure. Non-zero is the backpressure design working. */
  diagnosticLoss: number;
  /** Required events still sitting in the writer's one bounded pending batch. */
  pendingRequired: number;
  recording: Readonly<{accepting:boolean|null;state:number|null;pending:number|null;lastCommit:number|null;confirmationFailures:number|null;recoveredUnknown:number|null}>;
  capacity: Readonly<{
    queueCapacity: number | null; queueUsed: number | null;
    observedAt: number | null; collectionFailed: number | null;
    databaseBytes: number | null; walBytes: number | null;
    availableBytes: number | null; totalBytes: number | null;
    diskLow: number | null; walHigh: number | null;
  }>;
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

  const observed=(name:string):number|null=>samples.find(sample=>sample.name===`gateway_recording_${name}`)?.value??null;
  const accepting=observed("accepting_requests");
  const storage = (name: string): number | null => samples.find(sample => sample.name === `gateway_storage_${name}`)?.value ?? null;
  return {
    capacity: { queueCapacity: observed("queue_capacity"), queueUsed: observed("queue_used"),
      observedAt: storage("observed_at_ms"), collectionFailed: storage("collection_failed"),
      databaseBytes: storage("database_bytes"), walBytes: storage("wal_bytes"),
      availableBytes: storage("available_bytes"), totalBytes: storage("total_bytes"),
      diskLow: storage("disk_low"), walHigh: storage("wal_high") },
    recording:{accepting:accepting===1?true:accepting===0?false:null,state:observed("state"),pending:observed("pending_required"),lastCommit:observed("last_commit_ms"),confirmationFailures:observed("confirmation_failures_total"),recoveredUnknown:observed("recovered_unknown")},
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

/** Capacity warnings do not change request admission or delete retained history. */
export function capacityWarnings(capacity: GatewayCounters["capacity"], now: number): readonly string[] {
  const warnings: string[] = [];
  if (capacity.collectionFailed === 1) warnings.push("容量采样失败，保留上次观测");
  if (capacity.observedAt === null) warnings.push("存储容量尚未观测");
  else if (now - capacity.observedAt > 180_000) warnings.push("容量观测已超过 3 分钟");
  if (capacity.diskLow === 1) warnings.push("磁盘剩余空间偏低，请安排扩容或离线维护");
  if (capacity.walHigh === 1) warnings.push("WAL 较大，请检查长时间读取及写入积压");
  if (capacity.queueCapacity !== null && capacity.queueCapacity > 0 && capacity.queueUsed !== null && capacity.queueUsed / capacity.queueCapacity >= 0.8) warnings.push("记录队列占用已达 80%");
  return warnings;
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
