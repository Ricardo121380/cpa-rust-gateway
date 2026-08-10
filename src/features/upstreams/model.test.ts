import { describe, expect, it } from "vitest";
import {
  oauthPollIntervalMs,
  oauthStateBadge,
  parseOAuthCallback,
  safeExternalUrl,
  upstreamSubresources,
} from "./model";
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

describe("parseOAuthCallback", () => {
  const REDIRECT =
    "http://127.0.0.1:8085/callback?code=4%2F0AY0e-g7abc&state=st-9f3c&scope=openid";

  it("reads code and state out of the full redirect address", () => {
    const parsed = parseOAuthCallback(REDIRECT);
    expect(parsed).toEqual({
      ok: true,
      input: { state: "st-9f3c", code: "4/0AY0e-g7abc", callback_url: REDIRECT },
    });
  });

  it("accepts a bare query string, with or without the leading ?", () => {
    expect(parseOAuthCallback("?code=abc&state=st-1")).toEqual({
      ok: true,
      input: { state: "st-1", code: "abc" },
    });
    expect(parseOAuthCallback("code=abc&state=st-1")).toEqual({
      ok: true,
      input: { state: "st-1", code: "abc" },
    });
  });

  it("carries a provider error through instead of demanding a code", () => {
    const parsed = parseOAuthCallback("?error=access_denied&state=st-2");
    expect(parsed).toEqual({
      ok: true,
      input: { state: "st-2", error: "access_denied" },
    });
  });

  it("refuses a paste with no state — it cannot be bound to this session", () => {
    const parsed = parseOAuthCallback("http://localhost/cb?code=abc");
    expect(parsed.ok).toBe(false);
    expect(parsed.ok === false && parsed.reason).toContain("state");
  });

  it("refuses a paste with neither code nor error", () => {
    const parsed = parseOAuthCallback("http://localhost/cb?state=st-3");
    expect(parsed.ok).toBe(false);
    expect(parsed.ok === false && parsed.reason).toContain("code");
  });

  it("refuses an empty paste", () => {
    expect(parseOAuthCallback("   ").ok).toBe(false);
  });

  it("refuses lengths the contract would reject, rather than earning a 400", () => {
    expect(parseOAuthCallback(`http://x/cb?code=a&state=${"s".repeat(513)}`).ok).toBe(false);
    expect(parseOAuthCallback(`http://x/cb?code=${"a".repeat(20481)}&state=s`).ok).toBe(false);
  });
});

describe("oauthStateBadge", () => {
  it("covers every state the contract can return, including expired", () => {
    for (const state of ["pending", "complete", "cancelled", "failed", "expired"] as const) {
      expect(oauthStateBadge(state)).toBeTruthy();
    }
  });
});

describe("safeExternalUrl", () => {
  it("passes http and https through", () => {
    expect(safeExternalUrl("https://auth.example.com/o?x=1")).toBe(
      "https://auth.example.com/o?x=1",
    );
    expect(safeExternalUrl("http://127.0.0.1:8085/o")).toBe("http://127.0.0.1:8085/o");
  });

  it("refuses schemes that execute or exfiltrate when clicked", () => {
    // Payload kept boring on purpose: check.mjs bans the storage API names
    // anywhere in src/, string literals included, and that gate earns more
    // than a decorative test payload does.
    expect(safeExternalUrl("javascript:alert(1)")).toBeUndefined();
    expect(safeExternalUrl("data:text/html,<script>1</script>")).toBeUndefined();
    expect(safeExternalUrl("file:///etc/passwd")).toBeUndefined();
  });

  it("refuses non-URLs and absent values", () => {
    expect(safeExternalUrl("not a url")).toBeUndefined();
    expect(safeExternalUrl(null)).toBeUndefined();
    expect(safeExternalUrl(undefined)).toBeUndefined();
    expect(safeExternalUrl("")).toBeUndefined();
  });
});
