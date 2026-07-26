import { describe, expect, it } from "vitest";
import { normalizedMaxRedirects, referencingUpstreams, validatePortEntry } from "./model";

describe("normalizedMaxRedirects", () => {
  it("deny forces 0 regardless of input", () => {
    expect(normalizedMaxRedirects("deny", 3)).toBe(0);
  });
  it("revalidate clamps to 1-5", () => {
    expect(normalizedMaxRedirects("revalidate", 0)).toBe(1);
    expect(normalizedMaxRedirects("revalidate", 3)).toBe(3);
    expect(normalizedMaxRedirects("revalidate", 9)).toBe(5);
  });
});

describe("validatePortEntry", () => {
  it("accepts valid ports and rejects out-of-range", () => {
    expect(validatePortEntry("443")).toBeUndefined();
    expect(validatePortEntry("0")).toBeDefined();
    expect(validatePortEntry("65536")).toBeDefined();
    expect(validatePortEntry("https")).toBeDefined();
  });
});

describe("referencingUpstreams", () => {
  it("finds referencing upstream ids", () => {
    const upstreams = [
      { id: "a", egress_policy_id: "p1" },
      { id: "b", egress_policy_id: null },
      { id: "c", egress_policy_id: "p1" },
    ];
    expect(referencingUpstreams("p1", upstreams)).toEqual(["a", "c"]);
    expect(referencingUpstreams("p2", upstreams)).toEqual([]);
  });
});
