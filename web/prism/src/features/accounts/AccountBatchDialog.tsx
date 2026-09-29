import { useMutation, useQuery, isCancelledError, CancelledError } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { sameDeletionImpact, type DeletionImpact } from "./deletionImpact";
import { ConfigurationTaskReview } from "../config-versions/ConfigurationTaskReview";
import type { ConfigurationTask } from "../config-versions/configurationTask";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import type { NativeReceipt } from "./NativeAccountDialog";
import { RuntimeApplyNotice } from "./RuntimeApplyNotice";
import { accountActionOutcomeLabel, type AccountActionOutcome } from "./accountActionModel";

export type AccountTarget=Readonly<{id:string;native:boolean;nativeProvider?:string;upstreamId?:string;name:string;provider:string;enabled:boolean;revision:number}>;
export type AccountAction="enable"|"disable"|"remove";
const labels={enable:"启用",disable:"停用",remove:"删除"};
const pending=():AccountActionOutcome=>({kind:"pending"});
const unexecuted=():AccountActionOutcome=>({kind:"unexecuted"});
function knownRejection(cause:unknown):boolean { const error=asAppError(cause); return error.kind==="invalid_request"||error.kind==="conflict"||error.kind==="session_invalid"; }
const staleTarget=()=>({kind:"conflict" as const,code:"account_target_changed",message:"账号授权已变化，请重新选择后核对。",status:409});

export function AccountBatchDialog({targets,action,onClose,onCompleted}:Readonly<{targets:readonly AccountTarget[];action:AccountAction;onClose:()=>void;onCompleted:(notice:string,version?:ConfigVersionSummary)=>void}>) {
  const [outcomes,setOutcomes]=useState<readonly AccountActionOutcome[]>(targets.map(pending));
  const [taskReceipt,setTaskReceipt]=useState<ConfigurationTask>();
  const [reviewBusy,setReviewBusy]=useState(false);
  const [impactConfirmed,setImpactConfirmed]=useState(false);
  const context=useVersionStore(state=>state.context);
  const tracked=useVersionStore(state=>state.pending);
  const impacts=useQuery({queryKey:["account-delete-impact",tracked?.id??context?.configVersionId,tracked?.revision??context?.revision,targets.map(target=>target.id)],enabled:action==="remove"&&!!context,retry:false,queryFn:async()=>{
    const rows:DeletionImpact[]=[];for(const target of targets.filter(target=>!target.native))rows.push(await call<DeletionImpact>("getCredentialDeletionImpact",{path:{credential_id:target.id},headers:{"X-Config-Version":tracked?.id??context!.configVersionId}}));return rows;
  }});
  useEffect(()=>setImpactConfirmed(false),[impacts.dataUpdatedAt,context?.configVersionId,context?.revision]);
  const [completed,setCompleted]=useState(false);
  const [needsApply,setNeedsApply]=useState(false);
  const [nativeApplying,setNativeApplying]=useState(false);
  const [version,setVersion]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const change=useMutation({mutationFn:async(retry:boolean=false)=>{
    if(action==="remove"&&(!impactConfirmed||!impacts.data))throw new Error("请完整读取并确认删除影响。");
    if(targets.length<1||targets.length>20)throw new Error("一次最多处理 20 份授权。");
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    const needsChange=(target:AccountTarget)=>action==="remove"||target.enabled!==(action==="enable");
    let task:Awaited<ReturnType<typeof beginConfigurationTask>>|undefined=retry?taskReceipt:undefined;
    if(task){const observed=await task.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});if(observed.status!=="draft"||observed.revision!==task.revision())throw new Error("工作配置已变化，请先核对完整修改；不会重放已处理项目。");}
    type NativeObservation=Readonly<{id:string;provider:string;revision:number;enabled:boolean}>;
    let nativeObservation:readonly NativeObservation[]|undefined;
    const observeNative=async(target:AccountTarget)=>{
      if(nativeObservation===undefined){
        const collected:NativeObservation[]=[];let cursor:string|undefined;const cursors=new Set<string>();
        do {
          const page=await call<{items:readonly NativeObservation[];next_cursor:string|null}>("listNativeAccounts",{query:{limit:100,...(cursor?{cursor}:{})}});
          assertOwner();collected.push(...page.items);cursor=page.next_cursor??undefined;
          if(collected.length>10000||(cursor&&cursors.has(cursor)))throw new Error("原生账号清单未完整读取，未执行操作。");if(cursor)cursors.add(cursor);
        } while(cursor!==undefined);
        nativeObservation=collected;
      }
      return nativeObservation.find((row)=>row.id===target.id&&row.provider===target.nativeProvider);
    };
    const beginTask=async()=>{
      if(task!==undefined)return task;
      task=await beginConfigurationTask(`${labels[action]}账号`);
      assertOwner();
      setTaskReceipt(task);setWorkingId(task.version.id);setVersion({...task.version,revision:task.revision()});
      return task;
    };
    const next=retry?[...outcomes]:targets.map(pending); let ordinarySaved=retry&&targets.some((target,index)=>!target.native&&["saved_draft","saved_unapplied"].includes(next[index]?.kind??"")); let stopped=false;
    const record=(index:number,outcome:AccountActionOutcome)=>{next[index]=outcome;setOutcomes([...next]);};
    const stop=(index:number,outcome:AccountActionOutcome)=>{record(index,outcome);for(let later=index+1;later<next.length;later+=1)if(next[later]?.kind==="pending")next[later]=unexecuted();setOutcomes([...next]);stopped=true;};
    for(const [index,target] of targets.entries()) {
      if(retry&&! ["rejected","unexecuted"].includes(next[index]?.kind??""))continue;
      assertOwner();
      try {
        if(target.native) {
          const observed=await observeNative(target);
          if(observed===undefined||observed.revision!==target.revision||observed.enabled!==target.enabled)throw staleTarget();
          if(!needsChange(target)){record(index,{kind:"skipped"});continue;}
          const path={account_id:target.id};
          const receipt=action==="remove"?await call<NativeReceipt>("deleteNativeAccount",{path,query:{revision:target.revision}}):await call<NativeReceipt>("updateNativeAccount",{path,body:{revision:target.revision,enabled:action==="enable"}});
          assertOwner();
          if(receipt.runtime_applied) record(index,{kind:"applied"});
          else { record(index,{kind:"saved_unapplied"});setNeedsApply(true);stop(index,{kind:"saved_unapplied"}); }
        } else {
          const observed=await call<{id:string;upstream_id:string;revision:number;status:string}>("getCredential",{path:{credential_id:target.id},headers:{"X-Config-Version":task?.version.id??tracked?.id??context!.configVersionId}});
          assertOwner();
          const observedEnabled=observed.status!=="disabled";
          if(observed.id!==target.id||observed.upstream_id!==target.upstreamId||observed.revision!==target.revision||observedEnabled!==target.enabled)throw staleTarget();
          if(!needsChange(target)){record(index,{kind:"skipped"});continue;}
          const working=await beginTask();
          const current=await working.read<{id:string;upstream_id:string;revision:number;status:string}>("getCredential",{path:{credential_id:target.id}});
          const currentEnabled=current.status!=="disabled";
          if(current.id!==target.id||current.upstream_id!==target.upstreamId||current.revision!==target.revision||(action!=="remove"&&currentEnabled!==target.enabled))throw staleTarget();
          if(action==="remove"){
            const before=impacts.data?.find(impact=>impact.credential_id===target.id&&impact.upstream_id===target.upstreamId);
            const impact=await working.read<DeletionImpact>("getCredentialDeletionImpact",{path:{credential_id:target.id}});
            if(!before||!sameDeletionImpact(before,impact,targets.filter(target=>!target.native).map(target=>target.id)))throw staleTarget();
            await working.mutate("deleteCredential",{path:{credential_id:target.id},headers:{"X-Deletion-Impact-Review":impact.review_token}});
          }
          else await working.mutate("updateCredentialStatus",{path:{credential_id:target.id},body:{status:action==="enable"?"active":"disabled",credential_revision:target.revision}});
          ordinarySaved=true;record(index,{kind:"saved_unapplied"});
        }
      } catch(cause) {
        if(isCancelledError(cause))throw cause;
        stop(index,knownRejection(cause)?{kind:"rejected",detail:asAppError(cause).message}:{kind:"unconfirmed",detail:asAppError(cause).message});
      }
      if(stopped)break;
    }
    if(task&&ordinarySaved) {
      const savedVersion={...task.version,revision:task.revision()};setVersion(savedVersion);
      if(!stopped&&!task.autoApply) {
        for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind:"saved_draft"};
      } else if(!stopped) {
        try {
          const finished=await task.finish();setVersion(finished);
          for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind:"applied"};
        } catch(cause) {
          if(isCancelledError(cause))throw cause;
          try {
            task.assertOwner();const observed=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});task.assertOwner();setVersion(observed);
            const kind=observed.revision===task.revision()?(observed.status==="active"?"applied":"saved_unapplied"):"saved_application_unconfirmed";
            for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind,detail:kind==="saved_application_unconfirmed"?"工作配置已有后续变化；原保存回执保留":undefined};
          } catch(reconcileCause) { if(isCancelledError(reconcileCause))throw reconcileCause; for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind:"saved_application_unconfirmed",detail:asAppError(cause).message}; }
        }
      }
      setOutcomes([...next]);
    }
    return next;
  },onSuccess:()=>setCompleted(true)});
  const reconcile=useMutation({mutationFn:async()=>{
    const next=[...outcomes];let currentVersion:ConfigVersionSummary|undefined;
    const owner=useVersionStore.getState().selectionGeneration,session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    if(workingId){currentVersion=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId}});assertOwner();}
    let nativeRows:readonly {id:string;provider:string;revision:number;enabled:boolean}[]|undefined;
    for(const [index,target] of targets.entries()){
      if(next[index]?.kind!=="unconfirmed")continue;
      if(target.native){
        if(!nativeRows){const rows:{id:string;provider:string;revision:number;enabled:boolean}[]=[];let cursor:string|undefined;const cursors=new Set<string>();do{const page=await call<{items:typeof rows;next_cursor:string|null}>("listNativeAccounts",{query:{limit:100,...(cursor?{cursor}:{})}});assertOwner();rows.push(...page.items);cursor=page.next_cursor??undefined;if(rows.length>10000||(cursor&&cursors.has(cursor)))throw new Error("账号清单不完整，结果仍待确认。");if(cursor)cursors.add(cursor);}while(cursor);nativeRows=rows;}
        const row=nativeRows.find(row=>row.id===target.id&&row.provider===target.nativeProvider);
        if(action==="remove"?!row:row&&row.revision>target.revision&&row.enabled===(action==="enable"))next[index]={kind:"saved_application_unconfirmed",detail:"已回读当前原生账号状态；运行应用结果仍需核对，不重放写入"};
      }else if(currentVersion){
        try{const row=await call<{id:string;upstream_id:string;revision:number;status:string}>("getCredential",{path:{credential_id:target.id},headers:{"X-Config-Version":currentVersion.id}});assertOwner();
          if(action!=="remove"&&row.id===target.id&&row.upstream_id===target.upstreamId&&row.revision>target.revision&&(row.status!=="disabled")===(action==="enable"))next[index]={kind:"saved_application_unconfirmed",detail:"已回读工作配置中的目标状态；原动作及应用回执仍需核对"};
        }catch(cause){if(isCancelledError(cause))throw cause;if(action==="remove"&&asAppError(cause).status===404)next[index]={kind:"saved_application_unconfirmed",detail:"已确认工作配置中账号不存在；应用回执仍需核对，历史记录保留"};else throw cause;}
      }
    }
    assertOwner();return {next,currentVersion};
  },onSuccess:({next,currentVersion})=>{setOutcomes(next);if(currentVersion)setVersion(currentVersion);}});
  const review=useMutation({mutationFn:()=>call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}}),onSuccess:(next)=>onCompleted("请核对已保存的账号修改。",next)});
  const busy=change.isPending||review.isPending||reconcile.isPending||nativeApplying||reviewBusy;
  const close=()=>{if(busy)return;if(completed)onCompleted(needsApply?"账号修改已保存，运行配置暂未应用。":"账号操作已处理，请查看各项结果。",version);else onClose();};
  return <Sheet title={`${labels[action]} ${targets.length} 份授权`} description={action==="remove"?"从 CPAR 删除选中的账号授权及其连接，历史请求与账本保留。这不会注销上游账号；再次使用需要重新授权或导入。":"仅改变所选授权的启停状态，不改变其他渠道。"} layout="confirm" tone={action==="remove"?"danger":"default"} onEscape={close} busy={busy} footer={<><SheetDismissButton className="secondary" disabled={busy}>{completed?"完成":"取消"}</SheetDismissButton>{!completed?<button className={action==="remove"?"danger":"primary"} disabled={busy||(action==="remove"&&(!impacts.data||!impactConfirmed||impacts.isFetching))} onClick={()=>change.mutate(false)}>确认{labels[action]}</button>:null}</>}>
    {action==="remove"&&!completed?<section aria-label="删除影响预览">
      {impacts.isPending?<p role="status">正在读取完整删除影响…</p>:null}{impacts.isError?<p role="alert">{asAppError(impacts.error).message}<button type="button" onClick={()=>{setImpactConfirmed(false);void impacts.refetch();}}>重新读取影响</button></p>:null}
      {impacts.data?.map(impact=><article key={impact.credential_id}><h4>{impact.credential_id} · revision {impact.credential_revision}</h4><p>移除 {impact.removed_bindings.length} 个账号连接和 {impact.removed_egress_profiles} 个出口绑定。</p><ul>{impact.retained_endpoints.map(endpoint=><li key={endpoint.id}>保留共享接口 {endpoint.id}；删除本批次后承接账号 {endpoint.remaining_credentials.filter(id=>!targets.some(target=>!target.native&&target.id===id)).join("、")||"无；新请求将无法使用这个账号池"}</li>)}</ul><p>保留路由：{impact.retained_routes.join("、")||"无相关路由"}。保留客户端 Key：{impact.retained_client_keys.join("、")||"无相关 Key"}。</p></article>)}
      {targets.some(target=>target.native)?<p>所选原生账号将从独立账号池移除；共享渠道接口、路由和客户端权限保留。运行应用失败会阻断服务的新请求，需要核对全局应用回执。</p>:null}
      <p>历史请求、账本和历史配置保留。本地删除不会注销上游账号，也不会撤销上游授权；历史版本恢复这些账号需要再次确认。</p>
      <label className="check-row"><input type="checkbox" checked={impactConfirmed} disabled={!impacts.data||impacts.isFetching||busy} onChange={event=>setImpactConfirmed(event.target.checked)}/>已核对删除对象、移除的绑定与保留的共享资源</label>
    </section>:null}
    <div className="operation-summary" role="status"><span>{completed?"处理结果":change.isPending?"正在处理":"已选择"}</span><strong>{targets.length} 份授权</strong><small>{outcomes.filter(item=>item.kind!=="pending"&&item.kind!=="unexecuted").length} 项已处理</small></div>
    <ul className="operation-items" aria-label="账号操作结果">{targets.map((target,index)=><li key={`${target.native}:${target.id}`}><div><strong>{target.name}</strong><span>{target.provider}</span></div><p data-outcome={outcomes[index]?.kind??"unexecuted"}>{accountActionOutcomeLabel(outcomes[index]??unexecuted())}</p></li>)}</ul>
    {needsApply?<RuntimeApplyNotice onBusyChange={setNativeApplying} onApplied={()=>{setNeedsApply(false);setOutcomes((items)=>items.map((outcome,index)=>targets[index]?.native&&outcome.kind==="saved_unapplied"?{kind:"applied"}:outcome));}}/>:null}
    {completed&&taskReceipt&&version?.status!=="active"?<ConfigurationTaskReview task={taskReceipt} onBusyChange={setReviewBusy} onApplied={next=>{setVersion(next);setOutcomes(items=>items.map((outcome,index)=>!targets[index]?.native&&["saved_draft","saved_unapplied","saved_application_unconfirmed"].includes(outcome.kind)?{kind:"applied"}:outcome));}}/>:null}
    {change.isError?<p role="alert">{asAppError(change.error).message}</p>:null}
    {completed&&outcomes.some(outcome=>outcome.kind==="unconfirmed")?<button className="secondary" disabled={busy} onClick={()=>reconcile.mutate()}>只读取账号状态核对未知结果</button>:null}
    {reconcile.isError?<p role="alert">{asAppError(reconcile.error).message}</p>:null}
    {completed&&action!=="remove"&&!outcomes.some(outcome=>outcome.kind==="unconfirmed")&&outcomes.some(outcome=>["rejected","unexecuted"].includes(outcome.kind))?<button className="secondary" disabled={busy||needsApply} onClick={()=>change.mutate(true)}>仅重试未保存或未执行项目</button>:null}
    {review.isError?<p role="alert">{asAppError(review.error).message}</p>:null}
    {completed&&workingId&&!version?<button className="secondary" disabled={busy} onClick={()=>review.mutate()}>查看待应用的修改</button>:null}
  </Sheet>;
}
