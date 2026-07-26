import { describe, expect, it } from "vitest";
import { paramsToRange, rangeToParams, resolveBucket, resolvePreset } from "./timerange";

const NOW = 1785060000000;

describe("timerange url contract", () => {
  it("presets round-trip through params", () => {
    for (const preset of ["today", "24h", "7d", "30d"] as const) {
      const range = resolvePreset(preset, NOW);
      const back = paramsToRange(new URLSearchParams(rangeToParams(range)), NOW);
      expect(back).toEqual(range);
    }
  });

  it("custom round-trips exact bounds", () => {
    const range = { preset: "custom", from_ms: 100, to_ms: 200, bucket: "hour" } as const;
    const back = paramsToRange(new URLSearchParams(rangeToParams(range)), NOW);
    expect(back).toEqual(range);
  });

  it("invalid custom falls back to today", () => {
    const back = paramsToRange(new URLSearchParams({ range: "custom", from: "9", to: "1" }), NOW);
    expect(back.preset).toBe("today");
  });

  it("auto bucket: hour <=48h, day beyond", () => {
    expect(resolveBucket(resolvePreset("24h", NOW))).toBe("hour");
    expect(resolveBucket(resolvePreset("7d", NOW))).toBe("day");
  });
});
