import { describe, expect, it } from "vitest";
import { growthSince, readCounters, successRate } from "./metrics";

const P = "gateway_observability_";

function exposition(lines: readonly string[]): string {
  return [`# HELP ${P}events_total Gateway lifecycle events.`, ...lines].join("\n");
}

const FULL = exposition([
  `${P}events_total{kind="request"} 120`,
  `${P}events_total{kind="attempt"} 140`,
  `${P}events_total{kind="usage"} 118`,
  `${P}events_total{kind="health"} 3`,
  `${P}events_total{kind="diagnostic"} 9`,
  `${P}attempts_total{outcome="succeeded"} 131`,
  `${P}attempts_total{outcome="failed"} 9`,
  `${P}usage_tokens_total{kind="input"} 44000`,
  `${P}usage_tokens_total{kind="output"} 9100`,
  `${P}usage_tokens_total{kind="reasoning"} 2400`,
  `${P}usage_tokens_total{kind="cache_read"} 12000`,
  `${P}usage_tokens_total{kind="cache_creation"} 800`,
  `${P}usage_tokens_total{kind="cached"} 12800`,
  `${P}queue_admission_total{outcome="required_queue_full"} 0`,
  `${P}queue_admission_total{outcome="diagnostic_dropped"} 6`,
  `${P}queue_admission_total{outcome="sink_closed"} 0`,
  `${P}durable_events_total{outcome="required_quarantined"} 2`,
  `${P}durable_events_total{outcome="write_failed"} 0`,
  `${P}durable_pending_required 4`,
]);

describe("readCounters", () => {
  it("reads every family the gateway renders", () => {
    const counters = readCounters(FULL);
    expect(counters.events).toEqual({
      request: 120,
      attempt: 140,
      usage: 118,
      health: 3,
      diagnostic: 9,
    });
    expect(counters.eventsTotal).toBe(390);
    expect(counters.attempts).toEqual({ succeeded: 131, failed: 9, total: 140 });
    expect(counters.tokens.input).toBe(44000);
    expect(counters.tokens.cache_creation).toBe(800);
    expect(counters.pendingRequired).toBe(4);
  });

  it("surfaces only non-zero loss counters, Required before Diagnostic", () => {
    const counters = readCounters(FULL);
    expect(counters.loss.map((signal) => [signal.key, signal.value, signal.severity])).toEqual([
      ["required_quarantined", 2, "required"],
      ["diagnostic_dropped", 6, "diagnostic"],
    ]);
    expect(counters.requiredLoss).toBe(2);
    expect(counters.diagnosticLoss).toBe(6);
  });

  it("separates shed diagnostics from lost Required events", () => {
    // Diagnostics dropping under pressure is the backpressure design working;
    // it must not raise the same alarm as a lost Required event.
    const counters = readCounters(
      exposition([`${P}queue_admission_total{outcome="diagnostic_dropped"} 4210`]),
    );
    expect(counters.diagnosticLoss).toBe(4210);
    expect(counters.requiredLoss).toBe(0);
  });

  it("reports a clean pipeline as no loss at all", () => {
    const counters = readCounters(
      exposition([
        `${P}queue_admission_total{outcome="required_queue_full"} 0`,
        `${P}durable_events_total{outcome="write_failed"} 0`,
      ]),
    );
    expect(counters.loss).toEqual([]);
    expect(counters.requiredLoss).toBe(0);
    expect(counters.diagnosticLoss).toBe(0);
  });

  it("reads a fresh gateway that has emitted nothing as zeros, not gaps", () => {
    const counters = readCounters("");
    expect(counters.eventsTotal).toBe(0);
    expect(counters.attempts.total).toBe(0);
    expect(counters.tokens.output).toBe(0);
    expect(counters.loss).toEqual([]);
  });
});

describe("successRate", () => {
  it("is the attempt ratio", () => {
    expect(successRate(readCounters(FULL))).toBeCloseTo(131 / 140, 10);
  });

  it("is undefined before any attempt — not 0%, which reads as total failure", () => {
    expect(successRate(readCounters(""))).toBeUndefined();
  });
});

describe("growthSince", () => {
  const first = readCounters(FULL);
  const later = readCounters(
    exposition([
      `${P}events_total{kind="request"} 130`,
      `${P}events_total{kind="attempt"} 150`,
      `${P}events_total{kind="usage"} 118`,
      `${P}events_total{kind="health"} 3`,
      `${P}events_total{kind="diagnostic"} 9`,
      `${P}attempts_total{outcome="succeeded"} 139`,
      `${P}attempts_total{outcome="failed"} 11`,
    ]),
  );

  it("subtracts the first scrape of the visit", () => {
    expect(growthSince(first, later)).toEqual({ attempts: 10, failed: 2, events: 20 });
  });

  it("has no baseline on the first scrape", () => {
    expect(growthSince(undefined, first)).toBeUndefined();
  });

  it("withholds a delta when the gateway restarted and counters went backwards", () => {
    expect(growthSince(later, first)).toBeUndefined();
  });
});
