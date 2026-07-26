import { describe, expect, it } from "vitest";
import { paramsToRange, resolvePreset } from "./timerange";

// Regression guard for the pagination bug found by the browser smoke suite:
// a range derived from a quantised clock must be stable across renders, so
// query keys stay identical and infinite-query pages keep accumulating.
describe("quantised now keeps ranges stable", () => {
  const quantise = (ms: number, period = 60_000) => Math.floor(ms / period) * period;

  it("two renders within the same tick produce identical ranges", () => {
    const t1 = 1785060000123;
    const t2 = t1 + 4_000; // later render, same minute bucket
    const a = resolvePreset("24h", quantise(t1));
    const b = resolvePreset("24h", quantise(t2));
    expect(a).toEqual(b);
  });

  it("raw Date.now()-style values would differ (the bug this guards)", () => {
    const a = resolvePreset("24h", 1785060000123);
    const b = resolvePreset("24h", 1785060004123);
    expect(a).not.toEqual(b);
  });

  it("params round-trip stays stable under quantisation", () => {
    const now = quantise(1785060000123);
    const range = resolvePreset("today", now);
    expect(paramsToRange(new URLSearchParams({ range: "today" }), now)).toEqual(range);
  });
});
