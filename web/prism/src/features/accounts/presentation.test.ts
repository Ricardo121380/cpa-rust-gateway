import {describe,expect,it} from "vitest";
import {accountGroups,accountName,accountSource,protocolName} from "./presentation";
describe("account presentation",()=>{
  it("keeps six human-facing families and prefers observed identity",()=>{
    expect(accountGroups.map((g)=>g.id)).toEqual(["api","codex","claude","kimi","kiro","grok"]);
    expect(accountName({email:"alex@example.test",phone:null,username:"alex"},"p12-test-account")).toBe("alex@example.test");
    expect(accountName({email:null,phone:"+8613800138000",username:null},"autoreg-batch")).toBe("+8613800138000");
  });
  it("never dresses up hashes or source batches as a user",()=>{
    for(const name of ["p12-06-codex-bridge-credential","autoreg-batch-20260911","A8CD43F1","grok-6e97a9e7ce03e2dc","账号测试","验收账号"])expect(accountName(undefined,name)).toBeUndefined();
    expect(accountSource("autoreg-batch-20260911")).toBe("Autoreg");
    expect(accountName(undefined,"billing-owner")).toBeUndefined();
    expect(accountName(undefined,"+8613800138000")).toBeUndefined();
  });
  it("labels actual connection protocols instead of opaque endpoint counts",()=>{
    expect(protocolName("openai/responses")).toBe("Responses");
    expect(protocolName("openai/chat-completions")).toBe("Chat Completions");
    expect(protocolName("anthropic/messages")).toBe("Messages");
  });
});
