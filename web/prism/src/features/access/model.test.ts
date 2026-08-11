import { describe, expect, it } from "vitest";
import {
  displayKeyStatus,
  formatExpiry,
  formatLimits,
  parseLimits,
} from "./model";

const base = {
  id: "k1",
  access_group_id: "g1",
  prefix: "rgw_0123456789abcdef",
  status: "active",
} as const;

describe("displayKeyStatus", () => {
  it("derives expired at exactly the expiry instant (strict now < expires)", () => {
    const record = { ...base, expires_at_ms: 100 };
    expect(displayKeyStatus(record, 99)).toBe("active");
    expect(displayKeyStatus(record, 100)).toBe("expired");
  });

  it("terminal statuses win over expiry", () => {
    expect(displayKeyStatus({ ...base, status: "revoked", expires_at_ms: 1 }, 0)).toBe("revoked");
    expect(displayKeyStatus({ ...base, status: "disabled" }, 0)).toBe("disabled");
  });

  it("null/absent expiry never expires", () => {
    expect(displayKeyStatus({ ...base, expires_at_ms: null }, Number.MAX_SAFE_INTEGER)).toBe("active");
    expect(displayKeyStatus(base, Number.MAX_SAFE_INTEGER)).toBe("active");
  });
});

describe("formatExpiry", () => {
  it("labels absent expiry", () => {
    expect(formatExpiry(null)).toBe("永不过期");
    expect(formatExpiry(undefined)).toBe("永不过期");
  });
});

describe("parseLimits / formatLimits", () => {
  it("round-trips what the table renders", () => {
    const limits = { max_concurrency: 4, rpm: 600 };
    const text = formatLimits(limits);
    expect(text).toBe("max_concurrency=4 rpm=600");
    expect(parseLimits(text)).toEqual({ ok: true, limits });
  });

  it("treats empty as no limits, not as an error", () => {
    expect(parseLimits("   ")).toEqual({ ok: true, limits: {} });
    expect(formatLimits({})).toBe("");
  });

  it("tolerates any run of whitespace between pairs", () => {
    expect(parseLimits("a=1\n  b=2\tc=3")).toEqual({ ok: true, limits: { a: 1, b: 2, c: 3 } });
  });

  it("refuses values the contract would reject rather than earning a 400", () => {
    // integer >= 0 per AccessGroupInput.limits
    expect(parseLimits("rpm=-1").ok).toBe(false);
    expect(parseLimits("rpm=1.5").ok).toBe(false);
    expect(parseLimits("rpm=many").ok).toBe(false);
  });

  it("refuses malformed pairs", () => {
    for (const bad of ["rpm", "=4", "rpm=", "rpm=4=5"]) {
      const parsed = parseLimits(bad);
      if (bad === "rpm=4=5") {
        // "4=5" is not an integer — still refused, just by the value rule
        expect(parsed.ok).toBe(false);
      } else {
        expect(parsed.ok).toBe(false);
      }
    }
  });

  it("refuses a duplicate key instead of silently keeping the last", () => {
    const parsed = parseLimits("rpm=1 rpm=2");
    expect(parsed.ok).toBe(false);
    expect(parsed.ok === false && parsed.reason).toContain("重复");
  });

  it("refuses more entries than the contract allows", () => {
    const many = Array.from({ length: 17 }, (_, i) => `k${i}=1`).join(" ");
    expect(parseLimits(many).ok).toBe(false);
  });
});
