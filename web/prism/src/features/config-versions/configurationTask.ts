import { readAccountRestorationReview, restorationHeaders, type AccountRestorationReview } from "./accountRestoration";
import { CancelledError } from "@tanstack/react-query";
import { call, callRevisioned } from "../../api/client";
import { managementOperations, type ManagementOperationName, type ManagementRequest } from "../../generated/management-client";
import { useSessionStore } from "../../session/sessionStore";
import { beginConfigurationEdit } from "./beginEdit";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";
import { assertChangePage, type ConfigurationChange, type ConfigurationChangePage } from "./pendingChanges";
import {latestLifecycleEvent,type LifecycleEvent} from "./configurationLifecycle";

export type ConfigurationTaskReview = Readonly<{target:ConfigVersionSummary;active?:ConfigVersionSummary;event:string;changes:readonly ConfigurationChange[];priorChanges:readonly ConfigurationChange[];restoration:AccountRestorationReview}>;

async function readChanges(target:ConfigVersionSummary,active:ConfigVersionSummary|undefined,assertOwner:()=>void) {
  const rows:ConfigurationChange[]=[];let cursor:string|undefined;
  if(!active)return rows;
  do {
    const page=await call<ConfigurationChangePage>("compareConfigVersions",{path:{config_version_id:target.id},query:{base_id:active.id,limit:200,...(cursor?{cursor}:{})}});
    assertOwner();assertChangePage(page,active,target);rows.push(...page.items);cursor=page.next_cursor??undefined;
    if(cursor&&rows.length>=40_000)throw new Error("差异未完整读取，请在配置版本中分批核对；本次不会应用。");
  } while(cursor);
  return rows;
}

/** Never silently write into a second draft while another batch is tracked. */
export function assertPendingConfigurationAdmission(sourceId:string|undefined) {
  const pending=useVersionStore.getState().pending;
  if(pending&&sourceId!==pending.id)throw new Error("已有待应用配置，请先返回该工作草稿继续修改，避免创建另一批配置。");
}

/** A bounded user task edits one draft; intermediate writes never unmount its form or move scope. */
export async function beginConfigurationTask(description:string, expectedSource?:Readonly<{id:string;revision:string}>, completion:"automatic"|"deferred"="automatic") {
  const session=useSessionStore.getState().generation;
  const owner=useVersionStore.getState();
  if(owner.context?.status==="archived")throw new Error("当前查看的是历史配置，请返回当前配置后修改。");
  if(expectedSource&&(owner.context?.configVersionId!==expectedSource.id||owner.context.revision!==expectedSource.revision))throw new Error("当前配置已变化，请重新核对后操作。");
  const assertOwner=()=>{
    if(useSessionStore.getState().generation!==session||useVersionStore.getState().selectionGeneration!==owner.selectionGeneration)throw new CancelledError({silent:true});
  };
  // Continue the one tracked draft without moving the page or losing the task.
  // Its existing changes must be shown before any explicit application.
  const pending=owner.pending;
  const version=pending&&owner.context?.configVersionId!==pending.id
    ?await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:pending.id}})
    :await beginConfigurationEdit(description);
  assertOwner();
  if(version.status!=="draft")throw new Error("待应用配置已变化，请重新核对后操作。");
  if(expectedSource&&owner.context?.status==="draft"&&(version.id!==expectedSource.id||version.revision!==expectedSource.revision))throw new Error("草稿已变化，请重新核对后操作。");
  if(expectedSource&&owner.context?.status==="active"&&version.parent_id!==expectedSource.id)throw new Error("已发布配置已变化，请重新核对后操作。");
  const prior=!!pending||owner.context?.status==="draft";
  const active=prior?(await call<ConfigVersionSummary[]>("listConfigVersions")).find(row=>row.status==="active"):undefined;
  assertOwner();
  if(prior&&(version.parent_id??null)!==(active?.id??null))throw new Error("草稿基线与活动配置不同，请在配置版本中重新核对。");
  const priorChanges=prior?await readChanges(version,active,assertOwner):[];
  const autoApply=completion==="automatic"&&!prior;
  let revision=version.revision;
  useVersionStore.getState().rememberPending(version);
  const read=async<T>(operation:ManagementOperationName,request:ManagementRequest={})=>{
    assertOwner();
    const scoped=managementOperations[operation].parameters.some(parameter=>parameter.in==="header"&&parameter.name==="X-Config-Version");
    const value=await call<T>(operation,{...request,headers:{...request.headers,...(scoped?{"X-Config-Version":version.id}:{})}});
    assertOwner();return value;
  };
  const mutate=async<T>(operation:ManagementOperationName,request:ManagementRequest={})=>{
    assertOwner();
    const result=await callRevisioned<T>(operation,{...request,headers:{...request.headers,"X-Config-Version":version.id,"If-Match":revision}});
    assertOwner();
    const deviceState=(result.value as {state?:string}|null)?.state;
    const unchangedDevice=(operation==="pollKiroEnrollment"&&["pending","denied","expired","failed","cancelled"].includes(deviceState??""))
      ||(operation==="pollKimiEnrollment"&&["pending","denied","expired","failed","cancelled"].includes(deviceState??""));
    if(BigInt(result.revision.slice(4))<BigInt(revision.slice(4))||(!unchangedDevice&&result.revision===revision))throw new Error("修改结果的版本未推进，请重读核对。");
    revision=result.revision;
    useVersionStore.getState().rememberPending({...version,revision});
    return result.value;
  };
  const preview=async():Promise<ConfigurationTaskReview>=>{
    assertOwner();
    const versions=await call<ConfigVersionSummary[]>("listConfigVersions");
    assertOwner();
    const target=versions.find((row)=>row.id===version.id);
    const active=versions.find((row)=>row.status==="active");
    if(target?.status!=="draft"||(target.parent_id??null)!==(active?.id??null))throw new Error("当前配置已改变，修改仍保存在待应用配置中，请重新核对。");
    const audit=await call<readonly LifecycleEvent[]>("listManagementAuditEvents");
    assertOwner();
    const changes=await readChanges(target,active,assertOwner);
    const restoration=await readAccountRestorationReview(target,active?.id??null);assertOwner();
    return {target,active,changes,priorChanges,restoration,event:latestLifecycleEvent(audit)};
  };
  const acceptAuthorizationReceipt=async(receipt:Readonly<{state:string;config_version:string;started_revision:string;revision:string}>)=>{
    assertOwner();
    if(receipt.state!=="completed"||receipt.config_version!==version.id||receipt.started_revision!==revision)return false;
    const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:version.id}});
    assertOwner();
    // Only the exact retained write receipt plus current resource revision may advance this
    // task. A later unrelated edit requires a fresh complete review, never a write replay.
    if(current.status!=="draft"||current.revision!==receipt.revision)return false;
    revision=receipt.revision;useVersionStore.getState().rememberPending(current);return true;
  };
  const acceptReviewedDraft=async(review:ConfigurationTaskReview)=>{
    assertOwner();
    if(review.target.id!==version.id||review.target.status!=="draft")throw new Error("清单不属于当前工作草稿。");
    const current=await read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:version.id}});
    if(current.status!=="draft"||current.revision!==review.target.revision)throw new Error("核对后工作草稿已变化，请重新读取完整清单。");
    revision=current.revision;useVersionStore.getState().rememberPending(current);
  };
  const apply=async(review:ConfigurationTaskReview,restorationConfirmed=false):Promise<ConfigVersionSummary>=>{
    assertOwner();
    if(review.target.id!==version.id)throw new Error("核对清单不属于本次任务。");
    const valid=await call<{valid:boolean}>("validateConfigVersion",{path:{config_version_id:version.id}});
    assertOwner();
    if(!valid.valid)throw new Error("配置校验未通过，修改仍保存在待应用配置中。");
    const publication=await call<{active_config_version_id:string}>("publishConfigVersion",{path:{config_version_id:version.id},headers:{"If-Match":review.target.revision,"X-Expected-Active-Version":JSON.stringify(review.active?.id??null),"X-Expected-Lifecycle-Event":String(review.event),...restorationHeaders(review.restoration,restorationConfirmed)}});
    assertOwner();
    if(publication.active_config_version_id!==version.id)throw new Error("配置应用结果与本次修改不一致，请重新核对。");
    // The acknowledged publication is the durable commit receipt. Page queries re-read the
    // applied resources after selection; an unrelated read failure must not turn it into a retry.
    revision=review.target.revision;
    return {...review.target,status:"active"};
  };
  const finish=async():Promise<ConfigVersionSummary>=>{
    assertOwner();
    if(!autoApply)return {...version,revision};
    const review=await preview();
    if(review.target.revision!==revision)throw new Error("核对期间配置已变化，请重新读取完整清单后应用。");
    return apply(review);
  };
  return {version,autoApply,priorChanges,assertOwner,read,mutate,preview,apply,acceptAuthorizationReceipt,acceptReviewedDraft,finish,revision:()=>revision};
}

export type ConfigurationTask = Awaited<ReturnType<typeof beginConfigurationTask>>;
