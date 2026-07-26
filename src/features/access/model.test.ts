import { describe, expect, it } from "vitest";
import { displayKeyStatus, formatExpiry } from "./model";

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
