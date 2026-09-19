import { CancelledError, isCancelledError } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { useSessionStore } from "../../session/sessionStore";
import type { ConfigurationTask } from "./connectModel";
import type { CandidateRecord } from "./model";

export type ModelTaskReceipt = Readonly<{
  kind: "unchanged" | "saved_draft" | "saved_applied" | "saved_unapplied" | "saved_partial" | "unconfirmed";
  workingVersion: ConfigVersionSummary;
  acknowledgedWrites: number;
  message: string;
}>;
type Source=Readonly<{id:string;revision:string}>;
type ModelTaskOptions=Readonly<{expectedSource?:Source;probeUnchanged?:boolean}>;

const knownRejection=(cause:unknown)=>{
  const kind=asAppError(cause).kind;
  return kind==="invalid_request"||kind==="conflict"||kind==="session_invalid";
};

export function sameCandidate(left:CandidateRecord|undefined,right:CandidateRecord):boolean{
  if(!left)return false;
  const fields=["id","route_id","endpoint_id","upstream_model","credential_scope","transform_mode","enabled","priority","weight"] as const;
  if(fields.some(field=>left[field]!==right[field]))return false;
  const keys=new Set([...Object.keys(left.capability_override),...Object.keys(right.capability_override)]);
  return [...keys].every(key=>left.capability_override[key]===right.capability_override[key]);
}

async function observe(task:ConfigurationTask):Promise<ConfigVersionSummary>{
  task.assertOwner();
  const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});
  task.assertOwner();
  return version;
}

/** Read against the selected source first, so a true no-op never forks an active configuration. */
async function probeUnchanged<T>(work:(task:ConfigurationTask)=>Promise<T>):Promise<Readonly<{source:Source;value?:T;unchanged:boolean;version:ConfigVersionSummary}>>{
  const owner=useVersionStore.getState();
  const session=useSessionStore.getState().generation;
  const context=owner.context;
  if(!context||context.status==="archived")throw new Error("请选择可编辑的配置后继续。");
  const assertOwner=()=>{
    if(useSessionStore.getState().generation!==session||useVersionStore.getState().selectionGeneration!==owner.selectionGeneration)throw new CancelledError({silent:true});
  };
  const source={id:context.configVersionId,revision:context.revision};
  const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:source.id}});
  assertOwner();
  if(version.revision!==source.revision||version.status!==context.status)throw new Error("当前配置已变化，请重新核对后操作。");
  const mutationRequired=Symbol("model-mutation-required");
  const probe:ConfigurationTask={version,autoApply:version.status!=="draft",assertOwner,
    read:async <V,>(operation:ManagementOperationName,request:ManagementRequest={})=>{
      assertOwner();
      const value=await call<V>(operation,{...request,headers:{...request.headers,"X-Config-Version":source.id}});
      assertOwner();return value;
    },
    mutate:async <V,>()=>{throw mutationRequired as V;},
    finish:async()=>version,
    revision:()=>source.revision,
  };
  let value:T|undefined;
  let unchanged=false;
  try{value=await work(probe);unchanged=true;}
  catch(cause){if(cause!==mutationRequired)throw cause;}
  if(unchanged){
    const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:source.id}});
    assertOwner();
    if(current.revision!==source.revision||current.status!==version.status)throw new Error("当前配置已变化，请重新核对后操作。");
  }
  return {source,value,unchanged,version};
}

/** Model connection writes can persist in stages; never replay an acknowledged or uncertain stage. */
export async function runModelTask<T>(description:string, work:(task:ConfigurationTask)=>Promise<T>,options:ModelTaskOptions={}):Promise<Readonly<{value?:T;receipt:ModelTaskReceipt}>>{
  const probed=options.probeUnchanged?await probeUnchanged(work):undefined;
  if(probed?.unchanged)return {value:probed.value,receipt:{kind:"unchanged",workingVersion:probed.version,acknowledgedWrites:0,message:"该模型来源已存在，本次没有修改配置。"}};
  const task=await beginConfigurationTask(description,options.expectedSource??probed?.source);
  let attempted=false;
  let acknowledgedWrites=0;
  let acknowledgedRevision=task.revision();
  const wrapped:ConfigurationTask={...task,mutate:async <V,>(operation:ManagementOperationName,request:ManagementRequest={})=>{
    attempted=true;
    const result=await task.mutate<V>(operation,request);
    acknowledgedWrites+=1;
    acknowledgedRevision=task.revision();
    return result;
  }};
  let value:T;
  try {value=await work(wrapped);}
  catch(cause){
    if(isCancelledError(cause))throw cause;
    if(!attempted||knownRejection(cause)&&acknowledgedWrites===0)throw cause;
    let workingVersion={...task.version,revision:acknowledgedRevision};
    try{workingVersion=await observe(task);}catch(readCause){if(isCancelledError(readCause))throw readCause;}
    const partial=knownRejection(cause)&&acknowledgedWrites>0;
    return {receipt:{kind:partial?"saved_partial":"unconfirmed",workingVersion,acknowledgedWrites,
      message:partial?`已有 ${acknowledgedWrites} 步保存，后续操作被拒绝：${asAppError(cause).message}。请核对工作配置；不要重放整批。`:`修改结果未确认：${asAppError(cause).message}。请核对工作配置；不要重复提交。`}};
  }
  if(acknowledgedWrites===0)return {value,receipt:{kind:task.autoApply?"saved_unapplied":"unchanged",workingVersion:task.version,acknowledgedWrites,message:task.autoApply?"已建立工作草稿，但没有资源写入；请核对该草稿。":"该模型来源已存在，本次没有修改配置。"}};
  if(!task.autoApply)return {value,receipt:{kind:"saved_draft",workingVersion:{...task.version,revision:task.revision()},acknowledgedWrites,message:"已保存到当前草稿，尚未应用。"}};
  try{
    const workingVersion=await task.finish();
    return {value,receipt:{kind:"saved_applied",workingVersion,acknowledgedWrites,message:"已保存并应用。"}};
  }catch(cause){
    if(isCancelledError(cause))throw cause;
    try{
      const workingVersion=await observe(task);
      if(workingVersion.revision===acknowledgedRevision&&workingVersion.status==="active")return {value,receipt:{kind:"saved_applied",workingVersion,acknowledgedWrites,message:"已保存并应用；发布回执已通过重读核对。"}};
      if(workingVersion.revision===acknowledgedRevision&&workingVersion.status==="draft")return {value,receipt:{kind:"saved_unapplied",workingVersion,acknowledgedWrites,message:"修改已保存，但应用未完成；请核对工作配置。"}};
      return {value,receipt:{kind:"unconfirmed",workingVersion,acknowledgedWrites,message:"工作配置随后发生变化，无法确认本次应用结果；请核对后继续。"}};
    }catch(readCause){
      if(isCancelledError(readCause))throw readCause;
      return {value,receipt:{kind:"unconfirmed",workingVersion:{...task.version,revision:acknowledgedRevision},acknowledgedWrites,message:`修改可能已保存，但应用结果未确认：${asAppError(cause).message}`}};
    }
  }
}
