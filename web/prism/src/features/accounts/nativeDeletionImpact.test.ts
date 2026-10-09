import {describe,it,expect} from "vitest";
import {sameNativeDeletionImpact,type NativeDeletionImpact} from "./deletionImpact";
const impact:NativeDeletionImpact={config_version:"active",config_revision:1,inventory_generation:4,account_id:"target",account_revision:2,provider:"build",review_token:"review",connections:[{upstream_id:"build-owner",service_name:"Build",upstream_enabled:true,endpoint_id:"build-channel",endpoint_enabled:true,models:[{route_id:"route",model:"model",upstream_model:"grok-model",candidate_enabled:true}]}],remaining_accounts:[{id:"batch-member",revision:0,enabled:true,auth_status:"active"},{id:"retained",revision:0,enabled:false,auth_status:"active"}],replacement_guaranteed:false,history_retained:true,shared_resources_retained:true,upstream_account_revoked:false};
describe("native deletion review",()=>{
  it("permits only a previously selected batch member to disappear",()=>{
    expect(sameNativeDeletionImpact(impact,{...impact,inventory_generation:5,review_token:"fresh",remaining_accounts:impact.remaining_accounts.slice(1)},["target","batch-member"])).toBe(true);
    expect(sameNativeDeletionImpact(impact,{...impact,remaining_accounts:[]},["target","batch-member"])).toBe(false);
  });
  it("rejects changed connection ownership, model reachability and target revision",()=>{
    expect(sameNativeDeletionImpact(impact,{...impact,account_revision:3},[])).toBe(false);
    expect(sameNativeDeletionImpact(impact,{...impact,connections:[{...impact.connections[0]!,upstream_id:"another"}]},[])).toBe(false);
    expect(sameNativeDeletionImpact(impact,{...impact,connections:[{...impact.connections[0]!,models:[]}]},[])).toBe(false);
  });
});
