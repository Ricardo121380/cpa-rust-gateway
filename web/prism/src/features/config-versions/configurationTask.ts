import { CancelledError } from "@tanstack/react-query";
import { call, callRevisioned } from "../../api/client";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";
import { useSessionStore } from "../../session/sessionStore";
import { beginConfigurationEdit } from "./beginEdit";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

/** A bounded user task edits one draft; intermediate writes never unmount its form or move scope. */
export async function beginConfigurationTask(description:string) {
  const session=useSessionStore.getState().generation;
  const owner=useVersionStore.getState();
  if(owner.context?.status==="archived")throw new Error("当前查看的是历史配置，请返回当前配置后修改。");
  const autoApply=owner.context?.status!=="draft";
  const assertOwner=()=>{
    if(useSessionStore.getState().generation!==session||useVersionStore.getState().selectionGeneration!==owner.selectionGeneration)throw new CancelledError({silent:true});
  };
  const version=await beginConfigurationEdit(description);
  assertOwner();
  let revision=version.revision;
  const read=<T>(operation:ManagementOperationName,request:ManagementRequest={})=>{
    assertOwner();return call<T>(operation,{...request,headers:{...request.headers,"X-Config-Version":version.id}});
  };
  const mutate=async<T>(operation:ManagementOperationName,request:ManagementRequest={})=>{
    assertOwner();
    const result=await callRevisioned<T>(operation,{...request,headers:{...request.headers,"X-Config-Version":version.id,"If-Match":revision}});
    assertOwner();
    if(BigInt(result.revision.slice(4))<=BigInt(revision.slice(4)))throw new Error("修改结果的版本未推进，请重读核对。");
    revision=result.revision;
    return result.value;
  };
  const finish=async():Promise<ConfigVersionSummary>=>{
    assertOwner();
    if(!autoApply)return {...version,revision};
    const versions=await call<ConfigVersionSummary[]>("listConfigVersions");
    assertOwner();
    const target=versions.find((row)=>row.id===version.id);
    const active=versions.find((row)=>row.status==="active");
    if(target?.status!=="draft"||target.revision!==revision||(target.parent_id??null)!==(active?.id??null))throw new Error("当前配置已改变，修改仍保存在待应用配置中，请重新核对。");
    const audit=await call<ReadonlyArray<{id:number;action:string}>>("listManagementAuditEvents");
    assertOwner();
    const valid=await call<{valid:boolean}>("validateConfigVersion",{path:{config_version_id:version.id}});
    assertOwner();
    if(!valid.valid)throw new Error("配置校验未通过，修改仍保存在待应用配置中。");
    const event=Math.max(0,...audit.filter((row)=>["config_published","config_rolled_back"].includes(row.action)).map((row)=>row.id));
    const publication=await call<{active_config_version_id:string}>("publishConfigVersion",{path:{config_version_id:version.id},headers:{"If-Match":revision,"X-Expected-Active-Version":JSON.stringify(active?.id??null),"X-Expected-Lifecycle-Event":String(event)}});
    assertOwner();
    if(publication.active_config_version_id!==version.id)throw new Error("配置应用结果与本次修改不一致，请重新核对。");
    // The acknowledged publication is the durable commit receipt. Page queries re-read the
    // applied resources after selection; an unrelated read failure must not turn it into a retry.
    return {...version,revision,status:"active"};
  };
  return {version,autoApply,assertOwner,read,mutate,finish};
}
