import { describe, expect, it } from "vitest";
import {
  enabledCapabilities,
  formatCapabilityOverride,
  parseCapabilityOverride,
  routeErrorLabel,
  toggleCapability,
  validCandidateParams,
  validRouteParams,
} from "./model";

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

describe("validCandidateParams", () => {
  it("enforces contract bounds (priority >= 0, weight 1..10000)", () => {
    expect(validCandidateParams(0, 1)).toBe(true);
    expect(validCandidateParams(5, 10000)).toBe(true);
    expect(validCandidateParams(-1, 1)).toBe(false);
    expect(validCandidateParams(0, 0)).toBe(false);
    expect(validCandidateParams(0, 10001)).toBe(false);
    expect(validCandidateParams(1.5, 1)).toBe(false);
  });
});

describe("parseCapabilityOverride", () => {
  it("treats empty as the valid override-nothing value", () => {
    // capability_override is REQUIRED, so `{}` must be expressible — an empty
    // box may not become "omit the field".
    expect(parseCapabilityOverride("   ")).toEqual({ ok: true, override: {} });
  });

  it("parses booleans only", () => {
    expect(parseCapabilityOverride("vision=true tools=false")).toEqual({
      ok: true,
      override: { vision: true, tools: false },
    });
  });

  it("rejects non-boolean values rather than coercing them", () => {
    const parsed = parseCapabilityOverride("vision=1");
    expect(parsed.ok).toBe(false);
  });

  it("rejects malformed tokens and duplicate keys", () => {
    expect(parseCapabilityOverride("vision").ok).toBe(false);
    expect(parseCapabilityOverride("=true").ok).toBe(false);
    expect(parseCapabilityOverride("vision=").ok).toBe(false);
    expect(parseCapabilityOverride("vision=true vision=false").ok).toBe(false);
  });

  it("enforces the contract's 32-entry ceiling", () => {
    const tokens = Array.from({ length: 33 }, (_, index) => `k${index}=true`).join(" ");
    expect(parseCapabilityOverride(tokens).ok).toBe(false);
  });

  it("round-trips through the display format", () => {
    const parsed = parseCapabilityOverride("a=true b=false");
    if (!parsed.ok) {
      throw new Error("expected a parse");
    }
    expect(parseCapabilityOverride(formatCapabilityOverride(parsed.override))).toEqual(parsed);
  });

  it("accepts keys outside SEMANTIC_CAPABILITIES", () => {
    // The contract restricts the VALUE type, not the key set. A checkbox grid
    // over our own capability list would have refused this.
    expect(parseCapabilityOverride("some_future_capability=true").ok).toBe(true);
  });
});

describe("routeErrorLabel", () => {
  it("labels the four codes validate_model_route emits", () => {
    for (const code of [
      "route_missing_active_candidate",
      "route_candidate_endpoint_missing",
      "route_candidate_endpoint_disabled",
      "route_candidate_missing_active_credential",
    ]) {
      expect(routeErrorLabel(code)).toBeTypeOf("string");
    }
  });

  it("returns undefined for unknown codes so the caller can show the raw string", () => {
    expect(routeErrorLabel("some_code_added_later")).toBeUndefined();
  });
});
