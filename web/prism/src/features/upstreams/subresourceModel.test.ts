import { describe, expect, it } from "vitest";
import { accountStatusLabel, authenticationLabel, endpointLabel, runtimeConnectionLabel, sameEndpointConfiguration, nativeAccountsForEndpoints, observedEndpointAccounts } from "./subresourceModel";
import type { NativeAccount } from "../accounts/NativeAccounts";

describe("provider workspace presentation", () => {
  it("includes native channel identities even with no ordinary credential bindings", () => {
    const base: NativeAccount = { id: "native-1", provider: "grok_build", enabled: true, auth_status: "active", revision: 1, import_batch_id: "autoreg", identity: {email:"member@example.test",phone:null,username:null} };
    const accounts = [base, {...base,id:"native-2",enabled:false}, {...base,id:"console",provider:"grok_console" as const}];
    expect(nativeAccountsForEndpoints(accounts, [{adapter_id:"grok.build.responses"}]).map(a=>a.id)).toEqual(["native-1","native-2"]);
    expect(nativeAccountsForEndpoints(accounts, [{adapter_id:"openai.responses"}])).toEqual([]);
  });

  it("counts observed connections only for the exact provider and endpoint", () => {
    const row = {provider_id:"build",channel_id:"responses",account_id:"member"};
    expect(observedEndpointAccounts([row,row,{...row,provider_id:"other"},{...row,channel_id:"other",account_id:"different"}],"build","responses")).toEqual(["member"]);
  });
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
