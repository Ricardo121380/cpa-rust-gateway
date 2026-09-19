import { describe, expect, it } from "vitest";
import { accountStatusLabel, authenticationLabel, endpointLabel, runtimeConnectionLabel, sameEndpointConfiguration } from "./subresourceModel";

describe("provider workspace presentation", () => {
  it("uses channel semantics instead of credential storage kinds", () => {
    expect(authenticationLabel({ authentication: "oauth", category: "codex" })).toBe("OAuth 授权");
    expect(authenticationLabel({ authentication: "api_key", category: "kimi" })).toBe("Kimi API Key");
    expect(authenticationLabel({ authentication: "api_key", category: "api" })).toBe("API Key / Token");
  });

  it("keeps unknown authentication evidence unknown", () => {
    expect(authenticationLabel({ authentication: null, category: "kimi" })).toBe("接入方式未确认");
    expect(authenticationLabel({ category: "codex" })).toBe("接入方式未确认");
  });

  it("keeps non-active account states distinct", () => {
    expect(accountStatusLabel("cooling")).toBe("冷却中");
    expect(accountStatusLabel("unauthorized")).toBe("需要重新授权");
  });

  it("presents a protocol and host as the interface identity", () => {
    expect(endpointLabel({ api_format: "openai/responses", base_url: "https://api.example.test/v1" })).toContain("api.example.test");
  });

  it("compares endpoint targets by contract fields instead of JSON key order", () => {
    const inventory = {
      id: "endpoint-a", upstream_id: "provider-a", adapter_id: "openai-compatible", api_format: "openai/responses",
      base_url: "https://api.example.test/v1", inference_path: "/responses", models_path: null, transport: "https", enabled: true,
    };
    const detail = {
      enabled: true, transport: "https", models_path: null, inference_path: "/responses", base_url: "https://api.example.test/v1",
      api_format: "openai/responses", adapter_id: "openai-compatible", upstream_id: "provider-a", id: "endpoint-a",
    };
    expect(JSON.stringify(inventory)).not.toBe(JSON.stringify(detail));
    expect(sameEndpointConfiguration(inventory, detail)).toBe(true);
    expect(sameEndpointConfiguration(inventory, { ...detail, inference_path: "/chat/completions" })).toBe(false);
  });

  it("labels connection counts as operational evidence", () => {
    expect(runtimeConnectionLabel(0, "loading")).toBe("正在读取运行连接");
    expect(runtimeConnectionLabel(0, "unavailable")).toBe("运行连接暂不可读取");
    expect(runtimeConnectionLabel(0, "partial")).toContain("已加载的运行调度");
    expect(runtimeConnectionLabel(2, "observed")).toBe("运行调度中观测到 2 个账号");
  });
});
