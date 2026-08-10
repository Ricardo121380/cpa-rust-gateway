import { describe, expect, it } from "vitest";
import {
  accountStatusTone,
  enabledConjunction,
  providerPool,
  type AccountPoolItem,
} from "./pools";

function row(over: Partial<AccountPoolItem> = {}): AccountPoolItem {
  return {
    provider_id: "up-a",
    provider_name: "上游 A",
    provider_kind: "openai-compatible",
    provider_enabled: true,
    egress_policy_id: "eg-direct",
    channel_id: "ch-1",
    adapter_id: "openai-compatible.responses",
    api_format: "openai/responses",
    transport: "http",
    channel_enabled: true,
    account_id: "acc-1",
    account_kind: "api_key",
    account_status: "active",
    account_revision: 0,
    binding_enabled: true,
    configured_enabled: true,
    priority: 0,
    weight: 1,
    concurrency: 4,
    route_ids: ["rt-1"],
    ...over,
  };
}

describe("providerPool", () => {
  it("collapses binding rows into channels, accounts and bindings", () => {
    const pool = providerPool(
      [
        row(),
        row({ account_id: "acc-2", account_kind: "oauth", account_status: "cooling", priority: 1 }),
        row({ channel_id: "ch-2", api_format: "anthropic/messages", account_id: "acc-1" }),
      ],
      "up-a",
    );
    expect(pool?.channels.map((c) => c.channel_id)).toEqual(["ch-1", "ch-2"]);
    expect(pool?.accounts.map((a) => a.account_id)).toEqual(["acc-1", "acc-2"]);
    expect(pool?.bindings).toHaveLength(3);
    expect(pool?.provider_name).toBe("上游 A");
    expect(pool?.egress_policy_id).toBe("eg-direct");
  });

  it("lists each account under a channel once, even across repeated rows", () => {
    const pool = providerPool([row(), row({ route_ids: ["rt-2"] })], "up-a");
    expect(pool?.channels[0]?.account_ids).toEqual(["acc-1"]);
    // both rows are still real bindings — dedup is per channel listing only
    expect(pool?.bindings).toHaveLength(2);
  });

  it("keeps keyset order rather than sorting, so the tables match pagination", () => {
    const pool = providerPool(
      [row({ channel_id: "ch-z" }), row({ channel_id: "ch-a" })],
      "up-a",
    );
    expect(pool?.channels.map((c) => c.channel_id)).toEqual(["ch-z", "ch-a"]);
  });

  it("ignores rows belonging to another provider", () => {
    const pool = providerPool([row(), row({ provider_id: "up-b", channel_id: "ch-9" })], "up-a");
    expect(pool?.channels).toHaveLength(1);
  });

  it("is undefined when the provider has no binding at all", () => {
    // The projection is binding-driven: an upstream whose endpoints are
    // unbound produces zero rows, which is not the same as "no upstream".
    expect(providerPool([row({ provider_id: "up-b" })], "up-a")).toBeUndefined();
    expect(providerPool([], "up-a")).toBeUndefined();
  });

  it("carries a null egress policy through instead of inventing one", () => {
    expect(providerPool([row({ egress_policy_id: null })], "up-a")?.egress_policy_id).toBeNull();
  });
});

describe("accountStatusTone", () => {
  it("gives every status in the operations vocabulary its own tone", () => {
    const tones = (["active", "cooling", "unauthorized", "disabled"] as const).map(
      accountStatusTone,
    );
    expect(new Set(tones).size).toBe(4);
  });

  it("does not fold cooling into unauthorized — one is a wait, one is a stop", () => {
    expect(accountStatusTone("cooling")).not.toBe(accountStatusTone("unauthorized"));
  });
});

describe("enabledConjunction", () => {
  it("is the static three-way conjunction the report defines", () => {
    expect(enabledConjunction({ provider_enabled: true, channel_enabled: true, binding_enabled: true })).toBe(true);
    for (const off of ["provider_enabled", "channel_enabled", "binding_enabled"] as const) {
      expect(
        enabledConjunction({
          provider_enabled: true,
          channel_enabled: true,
          binding_enabled: true,
          [off]: false,
        }),
      ).toBe(false);
    }
  });
});
