import {beforeEach,describe,expect,it} from "vitest";
import {call} from "./client";
import {useSessionStore} from "../session/sessionStore";
import {useVersionStore,type ConfigVersionSummary} from "../features/config-versions/versionStore";
import {resetFixturesForTest,loseNextDeviceCompletionResponseForTest,deviceCompletionSnapshotForTest,prepareApiConnectionMatchesForTest,apiConnectionSnapshotForTest} from "../dev/fixtures";
import {CONNECTION_PRESETS} from "../features/upstreams/connectionPresets";
import {matchingApiConnections,chooseApiConnection} from "../features/upstreams/apiProviderModel";

const scope={versionScoped:true,mutating:true} as const;
type Receipt={session_id:string;credential_id:string;state:string;revision:string};
type CredentialInventory={items:{credential:{id:string;kind:string;status:string};authentication:string|null;category:string;provider:string}[]};
type AccountInventory={items:{id:string;status:string;operations:string[];managed:{authentication:string|null;category:string;provider:string}}[]};

beforeEach(async()=>{
  resetFixturesForTest();
  useSessionStore.getState().unlock(`mgmt_${"a".repeat(40)}`,`csrf_${"b".repeat(40)}`);
  const versions=await call<ConfigVersionSummary[]>("listConfigVersions");
  const draft=versions.find(row=>row.status==="draft");
  if(!draft)throw new Error("fixture draft missing");
  useVersionStore.getState().select(draft);
});

describe("S/E/O fixture contract evidence",()=>{
  it.each(["kiro","kimi-coding"] as const)("retains the original %s receipt after commit then client loss",async channel=>{
    await call("createUpstream",{body:{id:"seo-device",name:"Synthetic device",kind:channel,enabled:true,tags:[],egress_policy_id:null}},scope);
    const start=channel==="kiro"?"startKiroEnrollment":"startKimiEnrollment",poll=channel==="kiro"?"pollKiroEnrollment":"pollKimiEnrollment";
    const started=await call<Receipt>(start,{path:{upstream_id:"seo-device"},body:{id:"seo-device-account"}},scope);
    const request={path:{upstream_id:"seo-device"},body:{id:"seo-device-account",session_id:started.session_id}};
    await call(poll,request,scope);
    loseNextDeviceCompletionResponseForTest(channel);
    await expect(call(poll,request,scope)).rejects.toMatchObject({kind:"network"});
    const saved=deviceCompletionSnapshotForTest(started.session_id);
    expect(saved.receipt).toMatchObject({state:"completed",credential_id:"seo-device-account"});
    expect(saved.accounts).toHaveLength(1);expect(saved.exchanges).toBe(1);
    for(let index=0;index<2;index+=1){
      const recovered=await call<Receipt>("getAccountAuthorizationReceipt",{path:{session_id:started.session_id}},{versionScoped:true});
      expect(recovered).toMatchObject({session_id:started.session_id,state:"completed",credential_id:saved.receipt?.credential_id,revision:saved.receipt?.revision});
    }
    expect(deviceCompletionSnapshotForTest(started.session_id)).toEqual(saved);
    // The local ETag stayed at the pre-loss revision. Reads need no new exchange or If-Match.
    await expect(call(poll,request,scope)).rejects.toMatchObject({kind:"conflict"});
    expect(deviceCompletionSnapshotForTest(started.session_id)).toEqual(saved);
  });
  it.each(["responses","chat","messages","kimi","grok-api"])("keeps %s fixture connection identity distinct",presetId=>{
    const preset=CONNECTION_PRESETS.find(row=>row.id===presetId)!;
    prepareApiConnectionMatchesForTest(2,presetId);
    const {providers,endpoints}=apiConnectionSnapshotForTest();
    const matches=matchingApiConnections(providers,endpoints,preset);
    expect(matches).toHaveLength(2);expect(()=>chooseApiConnection(matches)).toThrow();
    expect(chooseApiConnection(matches,"batch-a-api-1-endpoint")?.provider.enabled).toBe(false);
    expect(matchingApiConnections(providers,endpoints,{...preset,format:preset.format==="openai/responses"?"openai/chat-completions":"openai/responses"})).toEqual([]);
  });
  it.each(["openai-compatible","anthropic-compatible","kimi-api","grok.official"])("rejects non API material for %s",async channel=>{
    const kind=channel==="kimi-api"?"kimi":channel;
    await call("createUpstream",{body:{id:"seo-api",name:"Synthetic API",kind,enabled:true,tags:[],egress_policy_id:null}},scope);
    for(const secret of ['{"refresh_token":"synthetic"}',"space in key","non-ascii-凭据"]){
      await expect(call("importChannelAccount",{path:{upstream_id:"seo-api"},body:{id:"seo-api-account",channel,secret}},scope)).rejects.toMatchObject({kind:"invalid_request"});
    }
    const inventory=await call<CredentialInventory>("listManagedCredentials",{query:{upstream_id:"seo-api"}},{versionScoped:true});
    expect(inventory.items).toEqual([]);
  });
  it("projects a completed Kiro bearer envelope as OAuth in both account inventories",async()=>{
    await call("createUpstream",{body:{id:"seo-kiro",name:"Kiro synthetic",kind:"kiro",enabled:true,tags:[],egress_policy_id:null}},scope);
    const started=await call<Receipt>("startKiroEnrollment",{path:{upstream_id:"seo-kiro"},body:{id:"seo-account"}},scope);
    const request={path:{upstream_id:"seo-kiro"},body:{id:"seo-account",session_id:started.session_id}};
    await call("pollKiroEnrollment",request,scope);
    await call("pollKiroEnrollment",request,scope);
    const accounts=await call<AccountInventory>("listAccountInventory",{query:{upstream_id:"seo-kiro"}},{versionScoped:true});
    const account=accounts.items.find(row=>row.id==="seo-account");
    expect(account?.status).toBe("enabled");
    expect(account?.managed).toMatchObject({authentication:"oauth",category:"kiro",provider:"Kiro"});
    expect(account?.operations).toContain("reauthorize");
    const inventory=await call<CredentialInventory>("listManagedCredentials",{query:{upstream_id:"seo-kiro"}},{versionScoped:true});
    expect(inventory.items.find(row=>row.credential.id==="seo-account")).toMatchObject({credential:{kind:"bearer",status:"active"},authentication:"oauth",category:"kiro",provider:"Kiro"});
  });
});
