import { describe, expect, it } from "vitest";
import { requestRange, requestSearchAfterFilter, selectedRequestPreset } from "./RequestHistory";

describe("requestSearchAfterFilter", () => {
  it("keeps an exact deep-linked range when an operator only changes filters", () => {
    const current = new URLSearchParams({
      tab: "requests",
      from_ms: "1700000000000",
      to_ms: "1700086400000",
    });
    const next = requestSearchAfterFilter(current, [["hours", "custom"], ["model", "grok-4.5"]]);

    expect(Object.fromEntries(next)).toEqual({
      tab: "requests",
      hours: "custom",
      model: "grok-4.5",
      from_ms: "1700000000000",
      to_ms: "1700086400000",
    });
  });

  it("uses a newly selected preset instead of carrying a stale exact range", () => {
    const current = new URLSearchParams({
      tab: "requests",
      from_ms: "1700000000000",
      to_ms: "1700086400000",
    });
    const next = requestSearchAfterFilter(current, [["hours", "168"], ["outcome", "failed"]]);

    expect(Object.fromEntries(next)).toEqual({ tab: "requests", hours: "168", outcome: "failed" });
  });

  it("sizes buckets from a valid exact range and rejects malformed endpoints", () => {
    expect(requestRange(new URLSearchParams({ from_ms: "0", to_ms: String(48 * 3_600_000) }), 999)).toEqual({
      from_ms: 0,
      to_ms: 48 * 3_600_000,
      bucket_ms: 86_400_000,
    });
    expect(requestRange(new URLSearchParams({ from_ms: "20", to_ms: "10", hours: "168" }), 1_000_000)).toEqual({
      from_ms: 1_000_000 - 168 * 3_600_000,
      to_ms: 1_000_000,
      bucket_ms: 86_400_000,
    });
  });

  it("labels an exact seven-day link as historical rather than a moving preset", () => {
    const range = new URLSearchParams({ from_ms: "1000", to_ms: String(1000 + 168 * 3_600_000) });
    expect(selectedRequestPreset(range)).toBe("custom");
    expect(requestSearchAfterFilter(range, [["hours", "custom"], ["outcome", "failed"]]).get("from_ms")).toBe("1000");
    expect(requestSearchAfterFilter(range, [["hours", "24"], ["outcome", "failed"]]).has("from_ms")).toBe(false);
  });

  it("labels a non-preset exact range and lets any selected preset replace it", () => {
    const range = new URLSearchParams({ from_ms: "1000", to_ms: String(1000 + 36 * 3_600_000) });
    expect(selectedRequestPreset(range)).toBe("custom");
    expect(requestSearchAfterFilter(range, [["hours", "custom"], ["outcome", "failed"]]).get("to_ms")).toBe(String(1000 + 36 * 3_600_000));
    expect(requestSearchAfterFilter(range, [["hours", "24"], ["outcome", "failed"]]).has("from_ms")).toBe(false);
  });

  it("keeps historical bounds custom even with a matching hours parameter", () => {
    const range = new URLSearchParams({ from_ms: "1000", to_ms: String(1000 + 24 * 3_600_000), hours: "24", outcome: "failed" });
    expect(selectedRequestPreset(range)).toBe("custom");
    const retained = requestSearchAfterFilter(range, [["hours", "custom"], ["outcome", "failed"]]);
    expect(retained.get("from_ms")).toBe("1000");
    const replaced = requestSearchAfterFilter(range, [["hours", "24"], ["outcome", "failed"]]);
    expect(replaced.has("from_ms")).toBe(false);
    expect(replaced.get("outcome")).toBe("failed");
  });
});
