import type {AccountActionOutcome,AccountActionTarget} from "./accountActionModel";

export type DeletionImpact=Readonly<{config_version:string;revision:string;credential_id:string;upstream_id:string;credential_revision:number;review_token:string;removed_bindings:readonly Readonly<{endpoint_id:string;enabled:boolean}>[];removed_egress_profiles:number;retained_endpoints:readonly Readonly<{id:string;enabled:boolean;remaining_credentials:readonly string[]}>[];retained_routes:readonly string[];retained_client_keys:readonly string[];history_retained:boolean;upstream_account_revoked:boolean}>;
export type NativeDeletionImpact=Readonly<{config_version:string;config_revision:number;inventory_generation:number;account_id:string;account_revision:number;provider:string;review_token:string;connections:readonly Readonly<{upstream_id:string;service_name:string;upstream_enabled:boolean;endpoint_id:string;endpoint_enabled:boolean;models:readonly Readonly<{route_id:string;model:string;upstream_model:string;candidate_enabled:boolean}>[]}>[];remaining_accounts:readonly Readonly<{id:string;revision:number;enabled:boolean;auth_status:string}>[];replacement_guaranteed:boolean;history_retained:boolean;shared_resources_retained:boolean;upstream_account_revoked:boolean}>;
function confirmedDeletionIds(targets:readonly AccountActionTarget[],outcomes:readonly AccountActionOutcome[],native:boolean):Set<string>{
  return new Set(targets.filter((target,index)=>target.native===native&&["applied","saved_unapplied","saved_draft"].includes(outcomes[index]?.kind??"")).map(target=>target.id));
}
/** Only a member with a confirmed successful DELETE may disappear; other accounts stay exact. */
export function sameNativeDeletionImpact(a:NativeDeletionImpact,b:NativeDeletionImpact,targets:readonly AccountActionTarget[],outcomes:readonly AccountActionOutcome[]):boolean{
  const removed=confirmedDeletionIds(targets,outcomes,true);
  if(b.remaining_accounts.some(row=>removed.has(row.id)))return false;
  const content=(impact:NativeDeletionImpact)=>({account_id:impact.account_id,account_revision:impact.account_revision,provider:impact.provider,connections:impact.connections,remaining_accounts:impact.remaining_accounts.filter(row=>!removed.has(row.id)),replacement_guaranteed:impact.replacement_guaranteed,history_retained:impact.history_retained,shared_resources_retained:impact.shared_resources_retained,upstream_account_revoked:impact.upstream_account_revoked});
  return JSON.stringify(content(a))===JSON.stringify(content(b));
}
/** Confirmed removals in this ordinary batch may disappear, but may not reappear. */
export function sameDeletionImpact(a:DeletionImpact,b:DeletionImpact,targets:readonly AccountActionTarget[],outcomes:readonly AccountActionOutcome[]):boolean{
  const removed=confirmedDeletionIds(targets,outcomes,false);
  if(b.retained_endpoints.some(endpoint=>endpoint.remaining_credentials.some(id=>removed.has(id))))return false;
  const content=(impact:DeletionImpact)=>({credential_id:impact.credential_id,upstream_id:impact.upstream_id,credential_revision:impact.credential_revision,removed_bindings:impact.removed_bindings,removed_egress_profiles:impact.removed_egress_profiles,retained_endpoints:impact.retained_endpoints.map(endpoint=>({...endpoint,remaining_credentials:endpoint.remaining_credentials.filter(id=>!removed.has(id))})),retained_routes:impact.retained_routes,retained_client_keys:impact.retained_client_keys,history_retained:impact.history_retained,upstream_account_revoked:impact.upstream_account_revoked});
  return JSON.stringify(content(a))===JSON.stringify(content(b));
}
