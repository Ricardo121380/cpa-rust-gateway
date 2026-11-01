// Behaviour first found against a real gateway: version-scoped reads on an empty gateway, the 100-item page
// cap, and validation answered as 409. The transport is faked; header rules are covered in client.test.ts.
import { beforeEach, describe, expect, it } from "vitest";
import { useTransport } from "../client";
import { store } from "../session";
import { ApiError, NO_CONFIG_VERSION } from "../errors";
import { get, getAll, validateVersion } from "../hooks";
import { resourceKey } from "@/lib/format";

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
let seen: Request[] = [];
function serve(f: (r: Request) => Response) { seen = []; useTransport((async (input: Request) => { seen.push(input); return f(input); }) as typeof fetch); }

beforeEach(() => {
  store.signOut();
  store.setSession({ kind: "admin", username: "admin", token: "session_" + "a".repeat(64), csrf: "csrf_" + "b".repeat(64), expiresAt: 0, mustChangePassword: false });
});

describe("version-scoped reads", () => {
  it("fail locally, without a request, when the gateway has no configuration version", async () => {
    serve(() => json([]));
    store.setConfig({ loaded: true });
    await expect(get("/admin/runtime/availability")).rejects.toMatchObject({ code: NO_CONFIG_VERSION });
    expect(seen).toHaveLength(0);
  });
  it("wait for the version list instead of sending a request without X-Config-Version", async () => {
    serve(() => json([]));
    const pending = get("/admin/runtime/availability");
    await Promise.resolve();
    expect(seen).toHaveLength(0);
    store.setConfig({ active: "cv-1", working: "cv-1", workingStatus: "active", revision: "rev-1", loaded: true });
    await pending;
    expect(seen[0].headers.get("X-Config-Version")).toBe("cv-1");
  });
  it("do not hold reads the contract does not scope to a version", async () => {
    serve(() => json({ items: [] }));
    await get("/admin/requests");
    expect(seen).toHaveLength(1);
  });
});

describe("getAll", () => {
  it("follows next_cursor with the contract's 100-item limit and reports truncation", async () => {
    serve((r) => {
      const cursor = Number(new URL(r.url).searchParams.get("cursor") ?? 0);
      return json({ observed_through_ms: 1, excluded_usage_events: 0, excluded_request_groups: 0, items: [cursor], next_cursor: String(cursor + 1) });
    });
    const page = await getAll<{ items: number[]; next_cursor: string | null }>("/admin/operations/usage", { query: { from_ms: 0 } }, 3);
    expect(seen.map((r) => new URL(r.url).searchParams.get("limit"))).toEqual(["100", "100", "100"]);
    expect(page.items).toEqual([0, 1, 2]);
    expect(page.truncated).toBe(true);
  });
  it("stops at the last page", async () => {
    serve(() => json({ items: [1, 2], next_cursor: null }));
    const page = await getAll<{ items: number[] }>("/admin/operations/usage");
    expect(page).toMatchObject({ items: [1, 2], truncated: false });
  });
});

describe("validateVersion", () => {
  it("reads the gateway's 409 management_lifecycle_conflict as an invalid graph", async () => {
    serve(() => json({ error: { code: "management_lifecycle_conflict", message: "x" } }, 409));
    await expect(validateVersion("draft-1")).resolves.toEqual({ valid: false, error_codes: ["management_lifecycle_conflict"] });
  });
  it("still surfaces other failures", async () => {
    serve(() => json({ error: { code: "management_internal_error", message: "x" } }, 500));
    await expect(validateVersion("draft-1")).rejects.toBeInstanceOf(ApiError);
  });
});

describe("resourceKey", () => {
  it("joins JSON-array key parts and leaves plain keys alone", () => {
    expect(resourceKey('["up-demo"]')).toBe("up-demo");
    expect(resourceKey('["rt-1","cd-2"]')).toBe("rt-1 / cd-2");
    expect(resourceKey("plain")).toBe("plain");
  });
});
