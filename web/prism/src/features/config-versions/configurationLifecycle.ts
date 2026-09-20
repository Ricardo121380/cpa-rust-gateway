import { CancelledError, isCancelledError } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

export type LifecycleMode = "publish" | "rollback";
export type LifecycleOwner = Readonly<{
  operationId:string; session:number; selection:number; contextId:string;
  source:ConfigVersionSummary;
}>;
export type LifecycleEvent = Readonly<{id:number|string;action:string;config_version_id:string;replaced_config_version_id?:string|null}>;
export type ReviewProof = Readonly<{targetRevision:string;baseId:string|null;baseRevision?:string}>;
export type PreparedLifecycle = Readonly<{
  owner:LifecycleOwner;mode:LifecycleMode;target:ConfigVersionSummary;
  activeId:string|null;eventId:string;validationOnly:boolean;
}>;
export type LifecycleReceipt = Readonly<{
  kind:"acknowledged"|"unconfirmed"|"rejected";
  prepared:PreparedLifecycle;message:string;
  publication?:Readonly<{active_config_version_id:string;replaced_config_version_id?:string|null}>;
}>;
export type LifecycleObservation = Readonly<{
  target:ConfigVersionSummary;activeId:string|null;latestEventId:string;
}>;

export function captureLifecycleOwner(source:ConfigVersionSummary):LifecycleOwner {
  const state=useVersionStore.getState();
  if(state.context?.configVersionId!==source.id||state.context.revision!==source.revision)throw new Error("请先进入并重新核对这份配置。");
  return {operationId:crypto.randomUUID(),session:useSessionStore.getState().generation,selection:state.selectionGeneration,contextId:source.id,source};
}
export function isLifecycleOwner(owner:LifecycleOwner):boolean {
  const state=useVersionStore.getState();
  return state.selectionGeneration===owner.selection&&state.context?.configVersionId===owner.contextId&&useSessionStore.getState().generation===owner.session;
}
export function assertLifecycleOwner(owner:LifecycleOwner):void {
  if(!isLifecycleOwner(owner))throw new CancelledError({silent:true});
}
export function latestLifecycleEvent(events:readonly LifecycleEvent[]):string {
  let latest=0n;
  for(const event of events){
    if(event.action!=="config_published"&&event.action!=="config_rolled_back")continue;
    if(typeof event.id==="number"&&!Number.isSafeInteger(event.id))throw new Error("生命周期审计编号无法精确读取。");
    if(!/^\d+$/u.test(String(event.id)))throw new Error("生命周期审计编号无效。");
    const value=BigInt(event.id);if(value>latest)latest=value;
  }
  return String(latest);
}
async function read<T>(owner:LifecycleOwner,operation:ManagementOperationName,request:ManagementRequest={}):Promise<T> {
  assertLifecycleOwner(owner);const value=await call<T>(operation,request);assertLifecycleOwner(owner);return value;
}
function exactSource(current:ConfigVersionSummary,source:ConfigVersionSummary):void {
  if(current.id!==source.id||current.revision!==source.revision||current.status!==source.status)throw new Error("配置状态或修订已变化，请重新查看变更并校验。");
}

/** Validation establishes provenance, never reserves publication rights. */
export async function prepareConfigurationLifecycle(owner:LifecycleOwner,mode:LifecycleMode,proof?:ReviewProof,validationOnly=false):Promise<PreparedLifecycle> {
  const source=await read<ConfigVersionSummary>(owner,"getConfigVersion",{path:{config_version_id:owner.source.id}});
  exactSource(source,owner.source);
  const versions=await read<ConfigVersionSummary[]>(owner,"listConfigVersions");
  const active=versions.find(version=>version.status==="active");
  const events=await read<LifecycleEvent[]>(owner,"listManagementAuditEvents");
  const eventId=latestLifecycleEvent(events);
  if(!validationOnly&&mode==="publish"){
    if(source.status!=="draft")throw new Error("只有草稿可以应用。");
    if(proof&&(proof.targetRevision!==source.revision||proof.baseId!==(active?.id??null)||(proof.baseRevision!==undefined&&proof.baseRevision!==active?.revision)))throw new Error("变更摘要已过期，请重新比较后应用。");
    if(source.parent_id!=null&&source.parent_id!==active?.id)throw new Error("此草稿来自较早的已发布配置，请重新核对来源。");
  }else if(!validationOnly&&(source.status!=="active"||active?.id!==source.id))throw new Error("回滚只能从当前活动配置发起。");
  let target=source;
  if(!validationOnly&&mode==="rollback"){
    const predecessor=events.filter(event=>event.config_version_id===source.id&&(event.action==="config_published"||event.action==="config_rolled_back"))
      .sort((a,b)=>BigInt(a.id)>BigInt(b.id)?-1:BigInt(a.id)<BigInt(b.id)?1:0)[0]?.replaced_config_version_id;
    const previous=versions.find(version=>version.id===predecessor&&version.status==="archived");
    if(!previous)throw new Error("没有服务端记录的一步回滚目标。");
    target=previous;
  }
  const validation=await read<{valid:boolean;error_codes?:readonly string[]}>(owner,"validateConfigVersion",{path:{config_version_id:target.id}});
  exactSource(await read<ConfigVersionSummary>(owner,"getConfigVersion",{path:{config_version_id:source.id}}),source);
  if(target.id!==source.id)exactSource(await read<ConfigVersionSummary>(owner,"getConfigVersion",{path:{config_version_id:target.id}}),target);
  if(!validation.valid)throw new Error(`配置校验未通过：${validation.error_codes?.join("、")||"请查看诊断"}`);
  return {owner,mode,target,activeId:active?.id??null,eventId,validationOnly};
}

/** One deliberate write with captured CAS. No retries or recovery POSTs. */
export async function commitConfigurationLifecycle(prepared:PreparedLifecycle):Promise<LifecycleReceipt> {
  const {owner,mode}=prepared;assertLifecycleOwner(owner);
  if(prepared.validationOnly)throw new Error("只读校验不能直接用于应用，请重新查看变更。");
  if(useVersionStore.getState().context?.revision!==owner.source.revision)throw new Error("校验后配置已变化，请重新查看变更。");
  try{
    const publication=await call<NonNullable<LifecycleReceipt["publication"]>>(mode==="publish"?"publishConfigVersion":"rollbackConfigVersion",{
      ...(mode==="publish"?{path:{config_version_id:owner.source.id}}:{}),
      headers:{"If-Match":owner.source.revision,"X-Expected-Active-Version":JSON.stringify(prepared.activeId),"X-Expected-Lifecycle-Event":prepared.eventId},
    });
    assertLifecycleOwner(owner);
    if(publication.active_config_version_id!==prepared.target.id|| (publication.replaced_config_version_id??null)!==prepared.activeId)throw new Error("应用响应与已确认目标不一致。");
    return {kind:"acknowledged",prepared,publication,message:mode==="publish"?"配置应用已确认。":"配置回滚已确认。"};
  }catch(cause){
    assertLifecycleOwner(owner);if(isCancelledError(cause))throw cause;
    const error=asAppError(cause);
    if(["invalid_request","conflict","session_invalid"].includes(error.kind))return {kind:"rejected",prepared,message:`本次操作被拒绝：${error.message}。请重新查看变更并确认，不会自动重放。`};
    return {kind:"unconfirmed",prepared,message:`应用结果未确认：${error.message}。仅可读取服务端状态核对，不会再次提交。`};
  }
}
export async function observeConfigurationLifecycle(receipt:LifecycleReceipt):Promise<LifecycleObservation> {
  const owner=receipt.prepared.owner;
  const target=await read<ConfigVersionSummary>(owner,"getConfigVersion",{path:{config_version_id:receipt.prepared.target.id}});
  if(target.id!==receipt.prepared.target.id)throw new Error("读取返回了另一份配置。");
  const versions=await read<ConfigVersionSummary[]>(owner,"listConfigVersions");
  const events=await read<LifecycleEvent[]>(owner,"listManagementAuditEvents");
  return {target,activeId:versions.find(version=>version.status==="active")?.id??null,latestEventId:latestLifecycleEvent(events)};
}
