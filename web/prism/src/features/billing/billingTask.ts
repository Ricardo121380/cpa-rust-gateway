import { CancelledError, isCancelledError } from "@tanstack/react-query";
import { call, callRevisioned } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useSessionStore } from "../../session/sessionStore";
import { assertPendingConfigurationAdmission, beginConfigurationTask } from "../config-versions/configurationTask";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { MAX_CATALOGS, isEffective, isPolicyUnset, type Catalog, type CatalogEntry, type ImportReceipt, type PricePolicy } from "./model";

export type BillingOwner=Readonly<{id:string;revision:string;status:"draft"|"active";selection:number;session:number}>;
export type BillingReceipt=Readonly<{
  kind:"catalog_saved_draft"|"catalog_applied"|"catalog_unapplied"|"policy_saved_draft"|"unconfirmed";
  target:string;
  workingVersion:ConfigVersionSummary;
  catalog?:ImportReceipt;
  observedCatalog?:boolean;
  message:string;
}>;
type CatalogAction=Readonly<
  |{kind:"import";target:string;effectiveAt:number;source:string;entries:readonly CatalogEntry[]}
  |{kind:"restore";target:string;effectiveAt:number;predecessor:Catalog}
>;
type PolicyAction=Readonly<{kind:"bind";catalogId:string}|{kind:"clear"}>;

export function captureBillingOwner():BillingOwner {
  const state=useVersionStore.getState();
  const context=state.context;
  if(!context||context.status==="archived")throw new Error("请选择当前配置或草稿后操作。");
  return {id:context.configVersionId,revision:context.revision,status:context.status,selection:state.selectionGeneration,session:useSessionStore.getState().generation};
}

export function isBillingOwner(owner:BillingOwner):boolean {
  const state=useVersionStore.getState();
  return state.selectionGeneration===owner.selection&&state.context?.configVersionId===owner.id
    &&useSessionStore.getState().generation===owner.session;
}

function assertOwner(owner:BillingOwner):void {
  if(!isBillingOwner(owner))throw new CancelledError({silent:true});
}

async function exactVersion(id:string,owner:BillingOwner):Promise<ConfigVersionSummary> {
  assertOwner(owner);
  const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:id}});
  assertOwner(owner);
  if(version.id!==id)throw new Error("重读返回了另一份配置，请保留当前回执。");
  return version;
}

async function sourceVersion(owner:BillingOwner,requireDraft:boolean):Promise<ConfigVersionSummary> {
  const version=await exactVersion(owner.id,owner);
  if(version.revision!==owner.revision||version.status!==owner.status||(requireDraft&&version.status!=="draft"))
    throw new Error("当前配置已变化，请重新读取后操作。");
  return version;
}

async function catalogs(owner:BillingOwner,versionId=owner.id):Promise<readonly Catalog[]> {
  assertOwner(owner);
  const rows=await call<readonly Catalog[]>("listBillingCatalogs",{headers:{"X-Config-Version":versionId}});
  assertOwner(owner);
  return rows;
}

function sameEntries(left:readonly CatalogEntry[],right:readonly CatalogEntry[]):boolean {
  if(left.length!==right.length)return false;
  const key=(entry:CatalogEntry)=>JSON.stringify([entry.provider_id,entry.channel_id,entry.model]);
  const lookup=new Map(left.map(entry=>[key(entry),entry]));
  return right.every(entry=>{
    const prior=lookup.get(key(entry));
    return prior!==undefined&&Object.keys(entry).every(field=>prior[field as keyof CatalogEntry]===entry[field as keyof CatalogEntry]);
  });
}

export async function runBillingCatalogTask(owner:BillingOwner,action:CatalogAction):Promise<BillingReceipt> {
  await sourceVersion(owner,false);
  const before=await catalogs(owner);
  if(before.length>=MAX_CATALOGS)throw new Error(`价格目录已达到 ${MAX_CATALOGS} 份上限，无法继续导入或恢复；现有目录保持可读。`);
  if(before.some(row=>row.catalog_version_id===action.target))throw new Error("目录版本标识已存在；目录只能新增，不能覆盖。");
  if(action.kind==="restore"){
    const observed=before.find(row=>row.catalog_version_id===action.predecessor.catalog_version_id);
    if(!observed||observed.effective_at_ms!==action.predecessor.effective_at_ms||observed.source!==action.predecessor.source||!sameEntries(observed.entries,action.predecessor.entries))
      throw new Error("来源目录已变化，请重新读取后恢复。");
  }
  await sourceVersion(owner,false);
  const task=await beginConfigurationTask(action.kind==="import"?"导入价格目录":"恢复价格目录",{id:owner.id,revision:owner.revision},"deferred");
  assertOwner(owner);
  let catalog:ImportReceipt;
  try {
    catalog=await task.mutate<ImportReceipt>(action.kind==="import"?"importBillingCatalog":"rollbackBillingCatalog",action.kind==="import"
      ?{body:{catalog_version_id:action.target,effective_at_ms:action.effectiveAt,source:action.source,entries:action.entries}}
      :{path:{catalog_version_id:action.predecessor.catalog_version_id},body:{new_catalog_version_id:action.target,effective_at_ms:action.effectiveAt}});
    task.assertOwner();
    if(catalog.catalog_version_id!==action.target||catalog.operation!==(action.kind==="import"?"imported":"rolled_back"))throw new Error("目录写入响应与目标不一致。");
  } catch(cause) {
    if(isCancelledError(cause))throw cause;
    const kind=asAppError(cause).kind;
    if(kind==="invalid_request"||kind==="conflict"||kind==="session_invalid")throw cause;
    let observedCatalog:boolean|undefined;
    try {
      const rows=await catalogs(owner,task.version.id);
      const found=rows.find(row=>row.catalog_version_id===action.target);
      observedCatalog=found!==undefined&&found.effective_at_ms===action.effectiveAt
        &&(action.kind==="restore"?sameEntries(found.entries,action.predecessor.entries):found.source===action.source&&sameEntries(found.entries,action.entries));
    }catch(readCause){if(isCancelledError(readCause))throw readCause;}
    let workingVersion=task.version;
    try{workingVersion=await exactVersion(task.version.id,owner);}catch(readCause){if(isCancelledError(readCause))throw readCause;}
    return {kind:"unconfirmed",target:action.target,workingVersion,observedCatalog,
      message:`目录写入回执未确认：${asAppError(cause).message}。${observedCatalog?"目录已可读，但重读不等于本次写入获确认。":""}请核对工作配置，不要重复提交。`};
  }
  const saved={...task.version,revision:task.revision()};
  useVersionStore.getState().rememberPending(saved);
  return {kind:"catalog_saved_draft",target:action.target,workingVersion:saved,catalog,
    message:"全局价格目录已保存，按生效时间参与计价；工作配置尚未发布，路由价格策略未自动改变。"};
}

async function policyValue(owner:BillingOwner):Promise<PricePolicy|null> {
  assertOwner(owner);
  try{const value=await call<PricePolicy>("getRoutingPricePolicy",{headers:{"X-Config-Version":owner.id}});assertOwner(owner);return value;}
  catch(cause){if(isPolicyUnset(cause)){assertOwner(owner);return null;}throw cause;}
}

export async function runBillingPolicyTask(owner:BillingOwner,baseline:PricePolicy|null,action:PolicyAction):Promise<BillingReceipt> {
  assertPendingConfigurationAdmission(owner.id);
  const version=await sourceVersion(owner,false);
  const observed=await policyValue(owner);
  if(observed?.catalog_version_id!==baseline?.catalog_version_id||observed?.comparison!==baseline?.comparison)
    throw new Error("路由价格策略已变化，请重新读取后操作。");
  if(action.kind==="clear"&&observed===null)throw new Error("此草稿未绑定价格目录，无需清除。");
  if(action.kind==="bind"){
    if(observed?.catalog_version_id===action.catalogId)throw new Error("当前草稿已绑定这份目录，无需重复保存。");
    const selected=(await catalogs(owner)).find(row=>row.catalog_version_id===action.catalogId);
    if(!selected||!isEffective(selected,Date.now()))throw new Error("目录不存在或尚未生效，请重新选择。");
  }
  await sourceVersion(owner,false);
  assertPendingConfigurationAdmission(owner.id);
  const task=owner.status==="active"?await beginConfigurationTask("维护路由价格策略",{id:owner.id,revision:owner.revision},"deferred"):undefined;
  const working=task?.version??version;
  const target=action.kind==="bind"?action.catalogId:baseline?.catalog_version_id??"";
  try{
    const operation=action.kind==="bind"?"setRoutingPricePolicy":"clearRoutingPricePolicy";
    const request=action.kind==="bind"?{body:{catalog_version_id:action.catalogId,comparison:"rate_dominance_v1"}}:{};
    const result=task?{value:await task.mutate<PricePolicy|undefined>(operation,request),revision:task.revision()}
      :await callRevisioned<PricePolicy|undefined>(operation,{...request,headers:{"X-Config-Version":owner.id,"If-Match":owner.revision}});
    assertOwner(owner);
    if(action.kind==="bind"&&result.value?.catalog_version_id!==action.catalogId)throw new Error("策略响应与目标目录不一致。");
    const saved={...working,revision:result.revision};
    useVersionStore.getState().rememberPending(saved);
    return {kind:"policy_saved_draft",target,workingVersion:saved,
      message:action.kind==="bind"?"价格策略已保存到草稿；发布后路由才会使用该目录。":"价格策略已从草稿清除；目录和历史账本均保留，发布后生效。"};
  }catch(cause){
    if(isCancelledError(cause))throw cause;
    const kind=asAppError(cause).kind;
    if(kind==="invalid_request"||kind==="conflict"||kind==="session_invalid")throw cause;
    let workingVersion=working;
    try{workingVersion=await exactVersion(working.id,owner);}catch(readCause){if(isCancelledError(readCause))throw readCause;}
    return {kind:"unconfirmed",target,workingVersion,
      message:`价格策略修改结果未确认：${asAppError(cause).message}。请核对当前草稿，不要重复提交。`};
  }
}

export async function reviewBillingVersion(owner:BillingOwner,receipt:BillingReceipt):Promise<ConfigVersionSummary> {
  return exactVersion(receipt.workingVersion.id,owner);
}
