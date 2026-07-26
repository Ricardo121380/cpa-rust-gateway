import { describe, expect, it } from "vitest";
import { oauthPollIntervalMs, upstreamSubresources } from "./model";
import type { ConfigVersionGraph } from "../../api/proposed-types";

const graph = {
  config_version: { id: "v", status: "draft", revision: "rev-1", created_at_ms: 0, description: "" },
  egress_policies: [],
  upstreams: [],
  endpoints: [
    { id: "e1", upstream_id: "a", adapter_id: "x", api_format: "openai/responses", base_url: "https://x", inference_path: "/r", transport: "https", enabled: true },
    { id: "e2", upstream_id: "b", adapter_id: "x", api_format: "openai/responses", base_url: "https://y", inference_path: "/r", transport: "https", enabled: true },
  ],
  credentials: [
    { id: "c1", upstream_id: "a", kind: "api_key", status: "active", revision: 0, secret_present: true },
  ],
  bindings: [
    { endpoint_id: "e1", upstream_id: "a", credential_id: "c1", enabled: true, priority: 0, weight: 1, concurrency: 1 },
  ],
  public_models: [],
  aliases: [],
  routes: [],
  candidates: [],
  access_groups: [],
  access_group_routes: [],
  client_keys: [],
} as unknown as ConfigVersionGraph;

describe("upstreamSubresources", () => {
  it("slices endpoints/credentials/bindings by upstream", () => {
    const a = upstreamSubresources(graph, "a");
    expect(a.endpoints.map((e) => e.id)).toEqual(["e1"]);
    expect(a.credentials.map((c) => c.id)).toEqual(["c1"]);
    expect(a.bindings).toHaveLength(1);
    const b = upstreamSubresources(graph, "b");
    expect(b.endpoints.map((e) => e.id)).toEqual(["e2"]);
    expect(b.credentials).toHaveLength(0);
  });
});

describe("oauthPollIntervalMs", () => {
  it("polls only while pending", () => {
    expect(oauthPollIntervalMs("pending")).toBe(2000);
    expect(oauthPollIntervalMs("complete")).toBe(false);
    expect(oauthPollIntervalMs(undefined)).toBe(false);
  });
});
