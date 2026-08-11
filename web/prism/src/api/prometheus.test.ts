import { describe, expect, it } from "vitest";
import { parsePrometheus, pick } from "./prometheus";

// The exact shape the gateway renders, asserted upstream in
// telemetry.rs::prometheus_rendering_uses_only_bounded_labels.
const EXPOSITION = `# HELP gateway_observability_events_total Gateway lifecycle events processed.
# TYPE gateway_observability_events_total counter
gateway_observability_events_total{kind="request"} 1
gateway_observability_events_total{kind="attempt"} 2
gateway_observability_events_total{kind="usage"} 3
# HELP gateway_observability_exports_total Export sink outcomes.
gateway_observability_exports_total{sink="json",outcome="emitted"} 10
gateway_observability_exports_total{sink="json",outcome="rejected"} 4
gateway_observability_exports_total{sink="opentelemetry",outcome="emitted"} 5
gateway_observability_durable_pending_required 7
`;

describe("parsePrometheus", () => {
  it("reads labelled and unlabelled samples, skipping comments and blanks", () => {
    const samples = parsePrometheus(EXPOSITION);
    expect(samples).toHaveLength(7);
    expect(samples[0]).toEqual({
      name: "gateway_observability_events_total",
      labels: { kind: "request" },
      value: 1,
    });
    expect(samples.at(-1)).toEqual({
      name: "gateway_observability_durable_pending_required",
      labels: {},
      value: 7,
    });
  });

  it("keeps every label of a multi-label sample", () => {
    const sample = parsePrometheus(EXPOSITION).find(
      (row) => row.name === "gateway_observability_exports_total",
    );
    expect(sample?.labels).toEqual({ sink: "json", outcome: "emitted" });
  });

  it("drops lines that would put a non-number on screen", () => {
    const text = [
      "good 1",
      "bad NaN",
      "also_bad",
      "   ",
      "# TYPE good counter",
      "trailing_timestamp 5 1785100000000",
    ].join("\n");
    const samples = parsePrometheus(text);
    expect(samples.map((row) => [row.name, row.value])).toEqual([
      ["good", 1],
      ["trailing_timestamp", 5],
    ]);
  });

  it("tolerates an empty body", () => {
    expect(parsePrometheus("")).toEqual([]);
  });
});

describe("pick", () => {
  const samples = parsePrometheus(EXPOSITION);

  it("totals a whole family when no label is given", () => {
    expect(pick(samples, "gateway_observability_events_total")).toBe(6);
  });

  it("selects one label value", () => {
    expect(pick(samples, "gateway_observability_events_total", { kind: "attempt" })).toBe(2);
  });

  it("matches a label subset across samples", () => {
    expect(pick(samples, "gateway_observability_exports_total", { outcome: "emitted" })).toBe(15);
    expect(pick(samples, "gateway_observability_exports_total", { sink: "json" })).toBe(14);
  });

  it("returns zero for names and labels the gateway did not emit", () => {
    expect(pick(samples, "gateway_observability_attempts_total", { outcome: "failed" })).toBe(0);
    expect(pick(samples, "no_such_metric")).toBe(0);
  });
});
