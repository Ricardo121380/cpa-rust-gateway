import { describe, expect, it } from "vitest";
import { importEndpointForChannel, importTargetForChannel, kiroImportRegion, preparedImportOperation, preparedImportRequest, selectedAccountProvider } from "./AddAccountDialog";

describe("account onboarding target ownership", () => {
  it("passes the one validated named-channel owner without requiring a hidden form field", () => {
    expect(importTargetForChannel(false, false, "kimi-coding", null)).toBe("kimi-coding");
    expect(importTargetForChannel(false, false, "codex-owned", "wrong-api-owner")).toBe("codex-owned");
    expect(importTargetForChannel(false, false, "", null)).toBe("");
  });

  it("never auto-binds a named account to an observed endpoint", () => {
    expect(importEndpointForChannel(false, "endpoint-observed")).toBe("");
    expect(importEndpointForChannel(true, "operator-selected")).toBe("operator-selected");
  });

  it("requires an explicit service selection even when one API service is configured", () => {
    const providers=[{id:"only-api"}];
    expect(selectedAccountProvider("",providers)).toBe("");
    expect(selectedAccountProvider("only-api",providers)).toBe("only-api");
    expect(selectedAccountProvider("stale-api",providers)).toBe("");
  });

  it("prepares Kiro imports from their transient declared region instead of cached provider rows", () => {
    expect(kiroImportRegion([{id:"one",label:"credential",secret:'{"kind":"enterprise","auth_region":"ap-southeast-1"}'}])).toBe("ap-southeast-1");
    expect(kiroImportRegion([{id:"one",label:"key",secret:"ksk_example"}])).toBe("us-east-1");
    expect(()=>kiroImportRegion([{id:"one",label:"a",secret:'{"auth_region":"us-east-1"}'},{id:"two",label:"b",secret:'{"auth_region":"eu-west-1"}'}])).toThrow("多个地区");
  });

  it("routes each built-in import to its channel-owned target preparation", () => {
    expect(preparedImportOperation("codex")).toBe("prepareCodexAccountTarget");
    expect(preparedImportOperation("claude")).toBe("prepareClaudeAccountTarget");
    expect(preparedImportOperation("kimi-coding")).toBe("prepareKimiAccountTarget");
    expect(preparedImportOperation("kiro")).toBe("prepareKiroAccountTarget");
    expect(preparedImportOperation("kimi-api")).toBeUndefined();
    expect(preparedImportRequest("prepareCodexAccountTarget", undefined)).toBeUndefined();
    expect(preparedImportRequest("prepareClaudeAccountTarget", undefined)).toBeUndefined();
    expect(preparedImportRequest("prepareKimiAccountTarget", undefined)).toBeUndefined();
    expect(preparedImportRequest("prepareKiroAccountTarget", "ap-southeast-1")).toEqual({body:{region:"ap-southeast-1"}});
  });
});
