import { beforeEach, describe, expect, it } from "vitest";
import { headersFor } from "../client";
import { store } from "../session";
import { describeError, ApiError } from "../errors";
import { remaining, windowLabel, tightest } from "@/features/accounts/quota";

describe("headersFor", () => {
  beforeEach(() => {
    store.signOut();
    store.setSession({ kind: "admin", username: "admin", token: "session_" + "a".repeat(64), csrf: "csrf_" + "b".repeat(64), expiresAt: 0, mustChangePassword: false });
    store.setConfig({ active: "cv-1", working: "draft-1", workingStatus: "draft", revision: "rev-7" });
  });
  it("adds key, CSRF, version and If-Match on a draft write", () => {
    const h = headersFor("PATCH", "/admin/upstreams/{upstream_id}", new Headers());
    expect(h["X-Management-Key"]).toMatch(/^session_/);
    expect(h["X-Management-CSRF-Token"]).toMatch(/^csrf_/);
    expect(h["X-Config-Version"]).toBe("draft-1");
    expect(h["If-Match"]).toBe('"rev-7"');
  });
  it("omits CSRF and If-Match on reads", () => {
    const h = headersFor("GET", "/admin/upstreams", new Headers());
    expect(h["X-Management-CSRF-Token"]).toBeUndefined();
    expect(h["If-Match"]).toBeUndefined();
    expect(h["X-Config-Version"]).toBe("draft-1");
  });
  it("does not attach the draft revision to a write aimed at another version", () => {
    const h = headersFor("POST", "/admin/config-versions/{config_version_id}/fork", new Headers({ "X-Config-Version": "cv-1" }));
    expect(h["X-Config-Version"]).toBeUndefined();
    expect(h["If-Match"]).toBeUndefined();
  });
  it("never sends X-Config-Version to operations that do not declare it", () => {
    expect(headersFor("GET", "/admin/requests", new Headers())["X-Config-Version"]).toBeUndefined();
  });
  it("sends no credentials when signed out", () => {
    store.signOut();
    expect(headersFor("GET", "/admin/system", new Headers())["X-Management-Key"]).toBeUndefined();
  });
});

describe("errors", () => {
  it("maps known codes to operator copy and keeps the code as detail", () => {
    const d = describeError(new ApiError(409, "management_version_not_writable", "x"));
    expect(d.title).toContain("草稿");
    expect(d.detail).toBe("management_version_not_writable · HTTP 409");
  });
  it("falls back to the server message for unknown codes", () => {
    expect(describeError(new ApiError(400, "something_new", "boom")).title).toBe("boom");
  });
});

describe("quota windows", () => {
  const w = (o: Partial<Parameters<typeof remaining>[0]>) => ({ name: "x", used_percent: null, reset_at: null, reset_at_ms: null, duration_seconds: null, used: null, limit: null, unit: null, ...o });
  it("derives labels from duration, not from position", () => {
    expect(windowLabel(w({ name: "primary_window", duration_seconds: 18000 }))).toBe("5 小时");
    expect(windowLabel(w({ name: "secondary_window", duration_seconds: 604800 }))).toBe("每周");
    expect(windowLabel(w({ name: "weird_name" }))).toBe("weird_name");
  });
  it("computes remaining from percent or used/limit", () => {
    expect(remaining(w({ used_percent: 25 }))).toBeCloseTo(0.75);
    expect(remaining(w({ used: 30, limit: 120 }))).toBeCloseTo(0.75);
    expect(remaining(w({}))).toBeNull();
  });
  it("picks the tightest window", () => {
    expect(tightest({ source: "codex", observed_at_ms: 0, windows: [w({ used_percent: 10 }), w({ used_percent: 90 })] })).toBeCloseTo(0.1);
  });
});
