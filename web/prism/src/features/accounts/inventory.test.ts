import {describe,it,expect} from "vitest";
import {groupManagedIdentities,type ManagedCredential} from "./inventory";
const record=(id:string,email:string|null,provider="Codex"):ManagedCredential=>({credential:{id,upstream_id:"upstream",kind:"oauth_json",status:"active",revision:0,secret_present:true},binding_count:1,identity:{email,phone:null,username:null},category:"codex",provider,connections:[]});
describe("contact identity grouping",()=>{
  it("keeps every exact authorization target within one observed identity",()=>{
    const groups=groupManagedIdentities([record("first","member@example.test"),record("second","member@example.test"),record("other","other@example.test")]);
    expect(groups).toHaveLength(2);expect(groups[0]!.map(r=>r.credential.id)).toEqual(["first","second"]);
  });
  it("does not merge missing identity, ambiguous names or different channels",()=>{
    const a=record("first",null);const b=record("second",null);
    expect(groupManagedIdentities([{...a,identity:{...a.identity,username:"Alex"}},{...b,identity:{...b.identity,username:"Alex"}}])).toHaveLength(2);
    expect(groupManagedIdentities([record("first","member@example.test","Codex"),record("second","member@example.test","ChatGPT")])).toHaveLength(2);
  });
});
