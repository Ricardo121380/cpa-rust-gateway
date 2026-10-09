import {describe,it,expect} from "vitest";
import {sameDeletionImpact,sameNativeDeletionImpact,type DeletionImpact,type NativeDeletionImpact} from "./deletionImpact";
import type {AccountActionOutcomeKind} from "./accountActionModel";
const impact:NativeDeletionImpact={config_version:"active",config_revision:1,inventory_generation:4,account_id:"target",account_revision:2,provider:"build",review_token:"review",connections:[{upstream_id:"build-owner",service_name:"Build",upstream_enabled:true,endpoint_id:"build-channel",endpoint_enabled:true,models:[{route_id:"route",model:"model",upstream_model:"grok-model",candidate_enabled:true}]}],remaining_accounts:[{id:"batch-member",revision:0,enabled:true,auth_status:"active"},{id:"retained",revision:0,enabled:false,auth_status:"active"}],replacement_guaranteed:false,history_retained:true,shared_resources_retained:true,upstream_account_revoked:false};
const targets=[{id:"target",native:true},{id:"batch-member",native:true}];
const pending=[{kind:"pending" as const},{kind:"pending" as const}];
const confirmed=[{kind:"pending" as const},{kind:"applied" as const}];
describe("native deletion review",()=>{
  it.each([
    ["revision",{...impact.remaining_accounts[0]!,revision:1}],
    ["enabled",{...impact.remaining_accounts[0]!,enabled:false}],
    ["auth",{...impact.remaining_accounts[0]!,auth_status:"reauth_required"}],
    ["disappearance",undefined],
  ] as const)("rejects %s drift in a selected member not yet deleted",(_label,member)=>{
    const next={...impact,remaining_accounts:member?[member,impact.remaining_accounts[1]!]:impact.remaining_accounts.slice(1)};
    expect(sameNativeDeletionImpact(impact,next,targets,pending)).toBe(false);
  });
  it("permits only a member with a successful deletion receipt to disappear",()=>{
    expect(sameNativeDeletionImpact(impact,{...impact,inventory_generation:5,review_token:"fresh",remaining_accounts:impact.remaining_accounts.slice(1)},targets,confirmed)).toBe(true);
    expect(sameNativeDeletionImpact(impact,{...impact,remaining_accounts:[]},targets,confirmed)).toBe(false);
  });
  it.each<AccountActionOutcomeKind>(["unconfirmed","unexecuted","rejected","saved_application_unconfirmed","skipped"])("does not treat %s as a confirmed DELETE",kind=>{
    expect(sameNativeDeletionImpact(impact,{...impact,remaining_accounts:impact.remaining_accounts.slice(1)},targets,[{kind:"pending"},{kind}])).toBe(false);
  });
  it("rejects a confirmed deleted member reappearing",()=>{
    expect(sameNativeDeletionImpact(impact,impact,targets,confirmed)).toBe(false);
    expect(sameNativeDeletionImpact(impact,{...impact,remaining_accounts:[{...impact.remaining_accounts[0]!,revision:1},impact.remaining_accounts[1]!] },targets,confirmed)).toBe(false);
  });
  it("keeps ordinary and native DELETE outcomes in their own namespace",()=>{
    expect(sameNativeDeletionImpact(impact,{...impact,remaining_accounts:impact.remaining_accounts.slice(1)},targets.map(target=>({...target,native:false})),confirmed)).toBe(false);
  });
  it("rejects changed connection ownership, model reachability and target revision",()=>{
    expect(sameNativeDeletionImpact(impact,{...impact,account_revision:3},targets,pending)).toBe(false);
    expect(sameNativeDeletionImpact(impact,{...impact,connections:[{...impact.connections[0]!,upstream_id:"another"}]},targets,pending)).toBe(false);
    expect(sameNativeDeletionImpact(impact,{...impact,connections:[{...impact.connections[0]!,models:[]}]},targets,pending)).toBe(false);
  });
});
describe("ordinary deletion review",()=>{
  const ordinary:DeletionImpact={config_version:"active",revision:"rev-1",credential_id:"target",upstream_id:"service",credential_revision:2,review_token:"review",removed_bindings:[{endpoint_id:"channel",enabled:true}],removed_egress_profiles:0,retained_endpoints:[{id:"channel",enabled:true,remaining_credentials:["batch-member","retained"]}],retained_routes:["route"],retained_client_keys:["key"],history_retained:true,upstream_account_revoked:false};
  const selected=targets.map(target=>({...target,native:false}));
  const after={...ordinary,retained_endpoints:[{...ordinary.retained_endpoints[0]!,remaining_credentials:["retained"]}]};
  it("allows only confirmed ordinary deletion results to remove prior bindings",()=>{
    expect(sameDeletionImpact(ordinary,after,selected,pending)).toBe(false);
    expect(sameDeletionImpact(ordinary,after,selected,[{kind:"pending"},{kind:"saved_unapplied"}])).toBe(true);
    expect(sameDeletionImpact(ordinary,after,selected,[{kind:"pending"},{kind:"unconfirmed"}])).toBe(false);
    expect(sameDeletionImpact(ordinary,after,targets,confirmed)).toBe(false);
  });
  it("rejects a previously deleted binding reappearing",()=>{
    expect(sameDeletionImpact(ordinary,ordinary,selected,confirmed)).toBe(false);
  });
});
