import { CancelledError, isCancelledError } from "@tanstack/react-query";
import { call, callRevisioned } from "../../api/client";
import { asAppError } from "../../api/errors";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import type { ModelTaskReceipt } from "./modelTask";
import type { RoutingPage } from "./model";

export type DraftRoutingOwner = Readonly<{id:string;revision:string;selection:number;session:number}>;
export type DraftRoutingRead = <T>(operation:ManagementOperationName,request?:ManagementRequest)=>Promise<T>;

export function captureModelActionOwner():DraftRoutingOwner {
  const state=useVersionStore.getState();
  const context=state.context;
  if(!context||context.status==="archived")throw new Error("请选择当前配置或草稿后操作。");
  return {id:context.configVersionId,revision:context.revision,selection:state.selectionGeneration,session:useSessionStore.getState().generation};
}

export function isModelActionOwner(owner:DraftRoutingOwner):boolean {
  const state=useVersionStore.getState();
  return useSessionStore.getState().generation===owner.session&&state.selectionGeneration===owner.selection&&state.context?.configVersionId===owner.id;
}

/** Complete bounded inventory, including unbound drafts, with one consistent source revision. */
export async function readRoutingInventory<T>(read:DraftRoutingRead,operation:"listRoutes"|"listRouteCandidates"|"listModelAliases"|"listManagedEndpoints",owner:DraftRoutingOwner):Promise<readonly T[]> {
  const items:T[]=[];
  const seen=new Set<string>();
  let cursor:string|undefined;
  do{
    const page=await read<RoutingPage<T>>(operation,{query:{limit:100,...(cursor?{cursor}:{})}});
    if(page.config_version!==owner.id||page.revision!==owner.revision)throw new Error("配置资源已变化，请重新读取后操作。");
    items.push(...page.items);
    if(items.length>10000)throw new Error("配置资源超过本次核对范围，请缩小操作范围。");
    cursor=page.next_cursor??undefined;
    if(cursor!==undefined){if(seen.has(cursor))throw new Error("配置资源分页未推进，请重新读取。");seen.add(cursor);}
  }while(cursor!==undefined);
  return items;
}

export function captureDraftRoutingOwner():DraftRoutingOwner {
  const state=useVersionStore.getState();
  const context=state.context;
  if(context?.status!=="draft")throw new Error("高级路由维护只允许在选定的草稿中进行。");
  return captureModelActionOwner();
}

/** One revision-bound draft mutation. The response is a receipt, never a retry instruction. */
export async function runDraftRoutingWrite<T>(owner:DraftRoutingOwner,operation:ManagementOperationName,request:ManagementRequest,verify:(read:DraftRoutingRead)=>Promise<void>):Promise<Readonly<{value?:T;receipt:ModelTaskReceipt}>> {
  const assertOwner=()=>{
    const state=useVersionStore.getState();
    if(useSessionStore.getState().generation!==owner.session||state.selectionGeneration!==owner.selection||state.context?.configVersionId!==owner.id)throw new CancelledError({silent:true});
  };
  const getVersion=async()=>{
    assertOwner();
    const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:owner.id}});
    assertOwner();
    if(version.status!=="draft"||version.revision!==owner.revision)throw new Error("草稿已变化，请重新读取后操作。");
    return version;
  };
  const read:DraftRoutingRead=async <V,>(name:ManagementOperationName,input:ManagementRequest={})=>{
    assertOwner();
    const value=await call<V>(name,{...input,headers:{...input.headers,"X-Config-Version":owner.id}});
    assertOwner();
    return value;
  };
  const version=await getVersion();
  await verify(read);
  await getVersion();
  assertOwner();
  try{
    const result=await callRevisioned<T>(operation,{...request,headers:{...request.headers,"X-Config-Version":owner.id,"If-Match":owner.revision}});
    assertOwner();
    if(result.revision===owner.revision)throw new Error("修改回执的修订号未推进，请核对草稿，不要重复提交。");
    return {value:result.value,receipt:{kind:"saved_draft",workingVersion:{...version,revision:result.revision},acknowledgedWrites:1,message:"已保存到当前草稿，尚未发布。"}};
  }catch(cause){
    if(isCancelledError(cause))throw cause;
    assertOwner();
    const kind=asAppError(cause).kind;
    if(kind==="invalid_request"||kind==="conflict"||kind==="session_invalid")throw cause;
    return {receipt:{kind:"unconfirmed",workingVersion:version,acknowledgedWrites:0,message:`修改结果未确认：${asAppError(cause).message}。请核对草稿，不要重复提交。`}};
  }
}
