import { describe, expect, it } from "vitest";
import { enabledCapabilities, toggleCapability, validRouteParams } from "./model";

describe("toggleCapability", () => {
  it("parallel_tools implies tools", () => {
    const next = toggleCapability({}, "parallel_tools", true);
    expect(next["parallel_tools"]).toBe(true);
    expect(next["tools"]).toBe(true);
  });

  it("disabling tools drops parallel_tools", () => {
    const next = toggleCapability({ tools: true, parallel_tools: true }, "tools", false);
    expect(next["tools"]).toBeUndefined();
    expect(next["parallel_tools"]).toBeUndefined();
  });

  it("plain toggles round-trip", () => {
    const on = toggleCapability({}, "vision", true);
    expect(enabledCapabilities(on)).toEqual(["vision"]);
    expect(enabledCapabilities(toggleCapability(on, "vision", false))).toEqual([]);
  });
});

describe("validRouteParams", () => {
  it("enforces contract bounds", () => {
    expect(validRouteParams(3, 30000)).toBe(true);
    expect(validRouteParams(0, 30000)).toBe(false);
    expect(validRouteParams(17, 30000)).toBe(false);
    expect(validRouteParams(3, 0)).toBe(false);
    expect(validRouteParams(3, 120001)).toBe(false);
  });
});
