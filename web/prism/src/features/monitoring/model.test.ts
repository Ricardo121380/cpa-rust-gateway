import { describe, expect, it } from "vitest";
import {
  costConfidenceLabel,
  costConfidenceTone,
  errorCodeLabel,
  errorScopeLabel,
  exactShare,
  formatMicrounits,
  formatPercent,
  parseFilters,
  parseTab,
  retryTone,
  stageLabel,
  summaryIsPartitioned,
  tally,
  type BillingSummary,
  type FailureRow,
} from "./model";

function summary(over: Partial<BillingSummary> = {}): BillingSummary {
  return {
    records: 10,
    exact_records: 6,
    partial_records: 2,
    unknown_records: 1,
    unpriced_records: 1,
    known_cost_microunits: 1234,
    ...over,
  };
}

function failure(over: Partial<FailureRow> = {}): FailureRow {
  return {
    provider_id: "prov-a",
    channel_id: "ch-a",
    account_id: "acct-a",
    request_id: "req-1",
    attempt_id: "att-1",
    ended_at_ms: 1_700_000_000_000,
    error_code: "ProviderRateLimited",
    error_scope: "provider",
    retry_decision: "retry_eligible",
    ...over,
  };
}

describe("exactShare", () => {
  it("is the share of records whose cost is exactly known", () => {
    expect(exactShare(summary())).toBeCloseTo(0.6);
    expect(formatPercent(exactShare(summary()))).toBe("60.0%");
  });

  it("returns null rather than NaN when there is nothing to divide", () => {
    expect(exactShare(summary({ records: 0 }))).toBeNull();
    expect(formatPercent(null)).toBe("—");
  });
});

describe("summaryIsPartitioned", () => {
  it("accepts a summary whose buckets add up", () => {
    expect(summaryIsPartitioned(summary())).toBe(true);
  });

  it("detects a summary whose buckets do NOT add up", () => {
    // The four buckets are declared as a partition of `records`. If that ever
    // breaks, showing the parts as if they summed would hide it.
    expect(summaryIsPartitioned(summary({ exact_records: 5 }))).toBe(false);
  });
});

describe("formatMicrounits", () => {
  it("never renders a currency symbol — the contract names no currency", () => {
    expect(formatMicrounits(1234567)).toBe("1,234,567");
    expect(formatMicrounits(1234567)).not.toMatch(/[$¥€]/u);
  });

  it("renders an absent cost as an em dash, not as zero", () => {
    expect(formatMicrounits(null)).toBe("—");
    expect(formatMicrounits(0)).toBe("0");
  });
});

describe("cost confidence", () => {
  it("labels and tones the four contract values apart", () => {
    expect(costConfidenceTone("exact")).toBe("good");
    expect(costConfidenceTone("partial")).toBe("warn");
    expect(costConfidenceTone("unknown")).toBe("muted");
    // "no rate in the catalog" is a real problem, not a neutral absence.
    expect(costConfidenceTone("unpriced")).toBe("serious");
  });

  it("renders an unknown member as itself instead of guessing", () => {
    expect(costConfidenceLabel("something_new")).toBe("something_new");
    expect(costConfidenceTone("something_new")).toBe("muted");
  });
});

describe("failure vocabularies", () => {
  it("labels error codes it knows and admits the ones it does not", () => {
    expect(errorCodeLabel("CredentialQuotaExceeded")).toBeTypeOf("string");
    expect(errorCodeLabel("SomeFutureCode")).toBeUndefined();
  });

  it("falls back to the raw scope rather than mislabelling it", () => {
    expect(errorScopeLabel("quota_window")).toBe("配额窗");
    expect(errorScopeLabel("brand_new_scope")).toBe("brand_new_scope");
  });

  it("separates a retry that may still have succeeded from one that did not", () => {
    expect(retryTone("completed")).toBe("good");
    expect(retryTone("retry_eligible")).toBe("warn");
    expect(retryTone("non_retryable")).toBe("critical");
    expect(retryTone("infrastructure_failure")).toBe("critical");
  });

  it("labels the eight attempt stages and passes unknown ones through", () => {
    expect(stageLabel("sse_bootstrap")).toBe("SSE 建流");
    expect(stageLabel("new_stage")).toBe("new_stage");
  });
});

describe("tally", () => {
  it("counts by a closed field, most frequent first", () => {
    const counts = tally(
      [
        failure({ error_code: "ProviderRateLimited" }),
        failure({ error_code: "ProviderRateLimited", attempt_id: "att-2" }),
        failure({ error_code: "EgressUnavailable", attempt_id: "att-3" }),
      ],
      "error_code",
    );
    expect(counts).toEqual([
      { key: "ProviderRateLimited", count: 2 },
      { key: "EgressUnavailable", count: 1 },
    ]);
  });

  it("breaks ties by key so equal counts do not reshuffle between renders", () => {
    const counts = tally(
      [failure({ error_scope: "provider" }), failure({ error_scope: "egress" })],
      "error_scope",
    );
    expect(counts.map((entry) => entry.key)).toEqual(["egress", "provider"]);
  });

  it("handles an empty stream", () => {
    expect(tally([], "retry_decision")).toEqual([]);
  });
});

describe("filters and tabs", () => {
  it("defaults to the ledger tab for anything unrecognised", () => {
    expect(parseTab(null)).toBe("ledger");
    expect(parseTab("nonsense")).toBe("ledger");
    expect(parseTab("failures")).toBe("failures");
  });

  it("drops a `status` value outside the closed cost-confidence enum", () => {
    // The contract's `status` selects COST CONFIDENCE, not request outcome —
    // forwarding "success" would earn a 400 that reads as a panel bug.
    const params = new Map([["status", "success"]]);
    expect(parseFilters(["status"], (key) => params.get(key) ?? null)).toEqual({});
  });

  it("keeps a legal cost-confidence value", () => {
    const params = new Map([["status", "unpriced"]]);
    expect(parseFilters(["status"], (key) => params.get(key) ?? null)).toEqual({
      status: "unpriced",
    });
  });

  it("trims and ignores blanks", () => {
    const params = new Map([
      ["provider_id", "  prov-a  "],
      ["channel_id", "   "],
    ]);
    expect(parseFilters(["provider_id", "channel_id"], (key) => params.get(key) ?? null)).toEqual({
      provider_id: "prov-a",
    });
  });
});
