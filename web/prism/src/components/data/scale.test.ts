import { describe, expect, it } from "vitest";
import { axisTicks, clamp, niceCeil, textWidth } from "./scale";

describe("axis scale", () => {
  it("rounds a maximum up to a clean top", () => {
    expect(niceCeil(0)).toBe(1);
    expect(niceCeil(-4)).toBe(1);
    expect(niceCeil(Number.NaN)).toBe(1);
    expect(niceCeil(7)).toBe(10);
    expect(niceCeil(180)).toBe(200);
    expect(niceCeil(0.03)).toBeCloseTo(0.05);
  });

  it("prefers a clean step over an exact tick count", () => {
    // 0/250/500/750/1000 reads; 0/230/460/690/920 does not.
    expect(axisTicks(920, 4)).toEqual([0, 250, 500, 750, 1000]);
    expect(axisTicks(206, 4)).toEqual([0, 100, 200, 300]);
    expect(axisTicks(0, 4)).toEqual([0, 1]);
  });

  it("keeps fractional steps free of float dust", () => {
    expect(axisTicks(0.055, 4)).toEqual([0, 0.02, 0.04, 0.06]);
  });
});

describe("tooltip sizing", () => {
  it("counts CJK glyphs as full-width", () => {
    expect(textWidth("请求", 10)).toBe(20);
    expect(textWidth("ab", 10)).toBeCloseTo(11.6);
  });

  it("clamps into range", () => {
    expect(clamp(5, 0, 3)).toBe(3);
    expect(clamp(-5, 0, 3)).toBe(0);
    expect(clamp(2, 0, 3)).toBe(2);
  });
});
