// Integration test: drives the real generated client + api layer + stores
// against the dev fixture backend (vitest env sets VITE_PRISM_FIXTURES=1).
// Exercises the exact invariants FE-1 depends on: version scoping, If-Match
// injection, ETag revision advance, 409 conflict marking, reveal-once shape.
import { beforeAll, describe, expect, it } from "vitest";
import { useSessionStore } from "../session/sessionStore";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../features/config-versions/versionStore";
import type { AppError } from "./errors";
import type { ClientKeyRecord, IssuedClientKey } from "../features/access/model";
import { call } from "./client";

const KEY = `mgmt_${"a".repeat(40)}`;
const CSRF = `csrf_${"b".repeat(40)}`;

beforeAll(() => {
  useSessionStore.getState().unlock(KEY, CSRF);
});

describe("fixture-backed api pipeline", () => {
  it("lists config versions and selects the draft", async () => {
    const versions = await call<ConfigVersionSummary[]>("listConfigVersions");
    expect(versions.length).toBeGreaterThanOrEqual(2);
    const draft = versions.find((version) => version.status === "draft");
    expect(draft).toBeDefined();
    useVersionStore.getState().select(draft as ConfigVersionSummary);
    expect(useVersionStore.getState().context?.revision).toBe("rev-4");
  });

  it("version-scoped list carries X-Config-Version and returns data", async () => {
    const keys = await call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true });
    expect(keys.some((record) => record.prefix.startsWith("rgw_"))).toBe(true);
  });

  it("issue advances the revision from the ETag and returns the one-time key", async () => {
    const before = useVersionStore.getState().context?.revision;
    const issued = await call<IssuedClientKey>(
      "issueClientKey",
      {
        body: {
          id: "key-test",
          access_group_id: "team-default",
          status: "active",
          expires_at_ms: null,
        },
      },
      { versionScoped: true, mutating: true },
    );
    expect(issued.key).toMatch(/^rgw_[0-9a-f]{16}_[0-9a-f]{64}$/u);
    const after = useVersionStore.getState().context?.revision;
    expect(after).not.toBe(before);
    expect(after).toBe("rev-5");
  });

  it("revoke returns 204 and still advances the revision", async () => {
    await call<undefined>(
      "revokeClientKey",
      { path: { client_key_id: "key-test" } },
      { versionScoped: true, mutating: true },
    );
    expect(useVersionStore.getState().context?.revision).toBe("rev-6");
    const keys = await call<ClientKeyRecord[]>("listClientKeys", {}, { versionScoped: true });
    expect(keys.find((record) => record.id === "key-test")?.status).toBe("revoked");
  });

  it("stale If-Match produces a conflict AppError and marks the store", async () => {
    const store = useVersionStore.getState();
    const context = store.context;
    expect(context).toBeDefined();
    // tamper the local revision to simulate a concurrent writer
    useVersionStore.setState({
      context: { ...(context as NonNullable<typeof context>), revision: "rev-1" },
    });
    await expect(
      call(
        "issueClientKey",
        {
          body: {
            id: "key-conflict",
            access_group_id: "team-default",
            status: "active",
            expires_at_ms: null,
          },
        },
        { versionScoped: true, mutating: true },
      ),
    ).rejects.toMatchObject({ kind: "conflict" } satisfies Partial<AppError>);
    expect(useVersionStore.getState().conflict).toBe(true);
  });
});

describe("option / contract agreement", () => {
  it("refuses an option the operation does not declare, instead of dropping it", async () => {
    // listOperationalUsage carries no X-Config-Version: usage is durable
    // observation of requests that already happened and spans config versions
    // by construction. Marking the call version-scoped used to do nothing at
    // all — no header, no error — which reads as "these numbers are filtered by
    // the version in the top bar" when they are not.
    await expect(
      call("listOperationalUsage", {}, { versionScoped: true }),
    ).rejects.toThrow(/declares no X-Config-Version/u);

    // Same for a revision guard on an operation that has none.
    await expect(
      call("validateConfigVersion", { path: { config_version_id: "draft-2026-08" } }, { mutating: true }),
    ).rejects.toThrow(/declares no If-Match/u);
  });

  it("still accepts the options where the contract does declare them", async () => {
    await expect(
      call("listOperationalAccountPools", {}, { versionScoped: true }),
    ).resolves.toBeDefined();
  });
});
