import { describe, expect, it, vi } from "vitest";
import { connectImportedAccount, prepareImportConnection, importEndpointForChannel, importTargetForChannel, kiroImportRegion, preparedImportOperation, preparedImportRequest, selectedAccountProvider } from "./AddAccountDialog";

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


describe("channel-owned import connections",()=>{
  function task(bindings: {credential_id:string;enabled?:boolean}[]=[]) {
    return {mutate:vi.fn().mockResolvedValue({upstream_id:"owned",endpoint_id:"prepared"}),read:vi.fn().mockResolvedValue(bindings)} as unknown as Parameters<typeof prepareImportConnection>[0];
  }
  it.each(["codex","claude","kimi-coding","kiro"])("connects %s to the prepared endpoint and returned credential",async(channel)=>{
    const tx=task();
    const target=await prepareImportConnection(tx,channel,"wrong-owner","wrong-endpoint","us-east-1");
    await connectImportedAccount(tx,target.endpoint_id,"deduplicated-credential");
    expect(target.upstream_id).toBe("owned");
    expect(tx.mutate).toHaveBeenLastCalledWith("createEndpointCredentialBinding",{path:{endpoint_id:"prepared"},body:{credential_id:"deduplicated-credential",enabled:true,priority:0,weight:1,concurrency:1}});
  });
  it("passes an explicitly selected Kimi Coding service to server validation",async()=>{
    const tx=task();
    await prepareImportConnection(tx,"kimi-coding","chosen-kimi","");
    expect(tx.mutate).toHaveBeenCalledWith("prepareKimiAccountTarget",{query:{upstream_id:"chosen-kimi"}});
  });
  it("preserves an existing disabled binding on duplicate import",async()=>{
    const tx=task([{credential_id:"existing",enabled:false}]);
    await connectImportedAccount(tx,"prepared","existing");
    expect(tx.mutate).not.toHaveBeenCalled();
  });
  it("retains API operator choice including save without connection",async()=>{
    const tx=task();
    expect(await prepareImportConnection(tx,"openai-compatible","operator-owner","" )).toEqual({upstream_id:"operator-owner",endpoint_id:""});
    await connectImportedAccount(tx,"","saved");
    expect(tx.read).not.toHaveBeenCalled();expect(tx.mutate).not.toHaveBeenCalled();
  });
  it("rejects incomplete preparation before importing",async()=>{
    const tx=task();vi.mocked(tx.mutate).mockResolvedValue({upstream_id:"owned"});
    await expect(prepareImportConnection(tx,"claude","","")).rejects.toThrow("不完整");
  });
  it("propagates binding conflicts without replay",async()=>{
    const tx=task();vi.mocked(tx.mutate).mockRejectedValue(new Error("conflict"));
    await expect(connectImportedAccount(tx,"prepared","saved")).rejects.toThrow("conflict");
    expect(tx.mutate).toHaveBeenCalledTimes(1);
  });
});
