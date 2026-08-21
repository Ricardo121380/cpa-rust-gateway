import { describe, expect, it } from "vitest";
import {
  groupNodesByPool,
  nodeReferences,
  poolReferences,
  targetIdSource,
  validTarget,
  validateProxyEndpoint,
  type EgressBinding,
  type ProxyNode,
  type ProxyPool,
} from "./compatible";

const POOLS: readonly ProxyPool[] = [
  { id: "pool-eu", upstream_id: "relay-a", name: "EU", enabled: true },
  { id: "pool-empty", upstream_id: "relay-a", name: "空", enabled: true },
];

function node(id: string, poolId: string | null): ProxyNode {
  return {
    id,
    upstream_id: "relay-a",
    pool_id: poolId,
    name: id,
    enabled: true,
    weight: 1,
    maximum_concurrency: 1,
    proxy_configured: true,
  };
}

function binding(kind: string, targetId: string | null): EgressBinding {
  return {
    endpoint_id: "ep-1",
    credential_id: "cred-1",
    target_kind: kind,
    target_id: targetId,
    failure_scope: "endpoint",
    stickiness: "none",
    pre_submit_max_attempts: 1,
  };
}

describe("validateProxyEndpoint", () => {
  it("accepts exactly a bare socks5 host:port", () => {
    expect(validateProxyEndpoint("socks5://127.0.0.1:1080")).toBeUndefined();
    expect(validateProxyEndpoint("socks5://proxy.internal:1080/")).toBeUndefined();
    expect(validateProxyEndpoint("  socks5://h:1080  ")).toBeUndefined();
  });

  it("names each rule the gateway would have rejected on", () => {
    // Mirrors UpstreamProxy::try_socks5. Without this the operator gets a bare
    // 400 with no indication of which rule they broke.
    expect(validateProxyEndpoint("http://h:1080")).toContain("socks5");
    expect(validateProxyEndpoint("socks5://user:pw@h:1080")).toContain("密码");
    expect(validateProxyEndpoint("socks5://h")).toContain("端口");
    expect(validateProxyEndpoint("socks5://h:1080/path")).toContain("路径");
    expect(validateProxyEndpoint("socks5://h:1080?a=1")).toContain("查询串");
    expect(validateProxyEndpoint("socks5://h:1080#f")).toContain("查询串");
    expect(validateProxyEndpoint("not a url")).toContain("URL");
    expect(validateProxyEndpoint("")).toContain("空");
  });
});

describe("target pairs", () => {
  it("draws fixed_proxy and proxy_pool ids from different lists", () => {
    // Getting this backwards produces a 400 the operator cannot read, because
    // both fields are just "an id" on the wire.
    expect(targetIdSource("fixed_proxy")).toBe("node");
    expect(targetIdSource("proxy_pool")).toBe("pool");
    expect(targetIdSource("direct")).toBe("none");
    expect(targetIdSource("tunnel")).toBe("unknown");
  });

  it("treats direct-with-an-id as invalid, not merely redundant", () => {
    // The backend matches on the pair: ("direct", Some(_)) is a 400, exactly
    // like ("proxy_pool", None).
    expect(validTarget("direct", "")).toBe(true);
    expect(validTarget("direct", "pool-eu")).toBe(false);
    expect(validTarget("proxy_pool", "pool-eu")).toBe(true);
    expect(validTarget("proxy_pool", "")).toBe(false);
    expect(validTarget("fixed_proxy", "node-1")).toBe(true);
    expect(validTarget("tunnel", "x")).toBe(false);
  });
});

describe("groupNodesByPool", () => {
  it("keeps a pool with no nodes visible", () => {
    // This is precisely the state a pool is in the moment it is created. A
    // grouping that dropped it would make the thing you just made invisible.
    const groups = groupNodesByPool(POOLS, [node("n1", "pool-eu")]);
    expect(groups.map((g) => g.pool?.id)).toEqual(["pool-eu", "pool-empty"]);
    expect(groups[1]?.nodes).toEqual([]);
  });

  it("gives pool-less nodes their own group, last", () => {
    const groups = groupNodesByPool(POOLS, [node("n1", "pool-eu"), node("loose", null)]);
    expect(groups.at(-1)?.pool).toBeUndefined();
    expect(groups.at(-1)?.nodes.map((n) => n.id)).toEqual(["loose"]);
  });

  it("does not lose a node whose pool_id points at nothing", () => {
    const groups = groupNodesByPool(POOLS, [node("orphan", "pool-deleted")]);
    expect(groups.at(-1)?.nodes.map((n) => n.id)).toEqual(["orphan"]);
  });

  it("adds no trailing group when every node has a real pool", () => {
    const groups = groupNodesByPool(POOLS, [node("n1", "pool-eu")]);
    expect(groups).toHaveLength(2);
  });
});

describe("delete blockers", () => {
  it("names both kinds of reference that would refuse a pool delete", () => {
    // The backend has no cascade — it refuses. Predicting it locally turns a
    // failed request into a sentence before the click.
    expect(
      poolReferences("pool-eu", [node("n1", "pool-eu")], [binding("proxy_pool", "pool-eu")]),
    ).toEqual(["节点 n1", "绑定 ep-1/cred-1"]);
    expect(poolReferences("pool-empty", [node("n1", "pool-eu")], [])).toEqual([]);
  });

  it("only counts fixed_proxy bindings against a node", () => {
    expect(nodeReferences("node-1", [binding("fixed_proxy", "node-1")])).toHaveLength(1);
    // Same id string, different namespace — a pool binding does not hold a node.
    expect(nodeReferences("node-1", [binding("proxy_pool", "node-1")])).toHaveLength(0);
  });
});
