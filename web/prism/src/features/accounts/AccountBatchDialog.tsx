import { useMutation, isCancelledError, CancelledError } from "@tanstack/react-query";
import { useState } from "react";
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
const labels={enable:"启用",disable:"停用",remove:"移除"};
const pending=():AccountActionOutcome=>({kind:"pending"});
const unexecuted=():AccountActionOutcome=>({kind:"unexecuted"});
function knownRejection(cause:unknown):boolean { const error=asAppError(cause); return error.kind==="invalid_request"||error.kind==="conflict"||error.kind==="session_invalid"; }
const staleTarget=()=>({kind:"conflict" as const,code:"account_target_changed",message:"账号授权已变化，请重新选择后核对。",status:409});

export function AccountBatchDialog({targets,action,onClose,onCompleted}:Readonly<{targets:readonly AccountTarget[];action:AccountAction;onClose:()=>void;onCompleted:(notice:string,version?:ConfigVersionSummary)=>void}>) {
  const [outcomes,setOutcomes]=useState<readonly AccountActionOutcome[]>(targets.map(pending));
  const [completed,setCompleted]=useState(false);
  const [needsApply,setNeedsApply]=useState(false);
  const [nativeApplying,setNativeApplying]=useState(false);
  const [version,setVersion]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const change=useMutation({mutationFn:async()=>{
    if(targets.length<1||targets.length>20)throw new Error("一次最多处理 20 份授权。");
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    const needsChange=(target:AccountTarget)=>action==="remove"||target.enabled!==(action==="enable");
    let task:Awaited<ReturnType<typeof beginConfigurationTask>>|undefined;
    type NativeObservation=Readonly<{id:string;provider:string;revision:number;enabled:boolean}>;
    let nativeObservation:readonly NativeObservation[]|undefined;
    const observeNative=async(target:AccountTarget)=>{
      if(nativeObservation===undefined){
        const collected:NativeObservation[]=[];let cursor:string|undefined;
        do {
          const page=await call<{items:readonly NativeObservation[];next_cursor:string|null}>("listNativeAccounts",{query:{limit:100,...(cursor?{cursor}:{})}});
          assertOwner();collected.push(...page.items);cursor=page.next_cursor??undefined;
        } while(cursor!==undefined);
        nativeObservation=collected;
      }
      return nativeObservation.find((row)=>row.id===target.id&&row.provider===target.nativeProvider);
    };
    const beginTask=async()=>{
      if(task!==undefined)return task;
      task=await beginConfigurationTask(`${labels[action]}账号`);
      assertOwner();
      setWorkingId(task.version.id);setVersion({...task.version,revision:task.revision()});
      return task;
    };
    const next=targets.map(pending); let ordinarySaved=false; let stopped=false;
    const record=(index:number,outcome:AccountActionOutcome)=>{next[index]=outcome;setOutcomes([...next]);};
    const stop=(index:number,outcome:AccountActionOutcome)=>{record(index,outcome);for(let later=index+1;later<next.length;later+=1)if(next[later]?.kind==="pending")next[later]=unexecuted();setOutcomes([...next]);stopped=true;};
    for(const [index,target] of targets.entries()) {
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
          const observed=await call<{id:string;upstream_id:string;revision:number;status:string}>("getCredential",{path:{credential_id:target.id}},{versionScoped:true});
          assertOwner();
          const observedEnabled=observed.status!=="disabled";
          if(observed.id!==target.id||observed.upstream_id!==target.upstreamId||observed.revision!==target.revision||observedEnabled!==target.enabled)throw staleTarget();
          if(!needsChange(target)){record(index,{kind:"skipped"});continue;}
          const working=await beginTask();
          const current=await working.read<{id:string;upstream_id:string;revision:number;status:string}>("getCredential",{path:{credential_id:target.id}});
          const currentEnabled=current.status!=="disabled";
          if(current.id!==target.id||current.upstream_id!==target.upstreamId||current.revision!==target.revision||(action!=="remove"&&currentEnabled!==target.enabled))throw staleTarget();
          if(action==="remove")await working.mutate("deleteCredential",{path:{credential_id:target.id}});
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
            const kind=observed.revision===task.revision()?(observed.status==="active"?"applied":"saved_unapplied"):"unconfirmed";
            for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind,detail:kind==="unconfirmed"?"工作配置已有后续变化":undefined};
          } catch(reconcileCause) { if(isCancelledError(reconcileCause))throw reconcileCause; for(const [index,target] of targets.entries())if(!target.native&&next[index]?.kind==="saved_unapplied")next[index]={kind:"unconfirmed",detail:asAppError(cause).message}; }
        }
      }
      setOutcomes([...next]);
    }
    return next;
  },onSuccess:()=>setCompleted(true)});
  const review=useMutation({mutationFn:()=>call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}}),onSuccess:(next)=>onCompleted("请核对已保存的账号修改。",next)});
  const busy=change.isPending||review.isPending||nativeApplying;
  const close=()=>{if(busy)return;if(completed)onCompleted(needsApply?"账号修改已保存，运行配置暂未应用。":"账号操作已处理，请查看各项结果。",version);else onClose();};
  return <Sheet title={`${labels[action]} ${targets.length} 份授权`} description={action==="remove"?"历史请求与费用保留；选中的授权和连接将被移除。":"仅改变所选授权的启停状态，不改变其他渠道。"} layout="confirm" tone={action==="remove"?"danger":"default"} onEscape={close} busy={busy} footer={<><SheetDismissButton className="secondary" disabled={busy}>{completed?"完成":"取消"}</SheetDismissButton>{!completed?<button disabled={busy} onClick={()=>change.mutate()}>确认{labels[action]}</button>:null}</>}>
    <div className="tablewrap"><table><thead><tr><th>账号</th><th>渠道</th><th>结果</th></tr></thead><tbody>{targets.map((target,index)=><tr key={`${target.native}:${target.id}`}><td>{target.name}</td><td>{target.provider}</td><td>{accountActionOutcomeLabel(outcomes[index]??unexecuted())}</td></tr>)}</tbody></table></div>
    {needsApply?<RuntimeApplyNotice onBusyChange={setNativeApplying} onApplied={()=>{setNeedsApply(false);setOutcomes((items)=>items.map((outcome,index)=>targets[index]?.native&&outcome.kind==="saved_unapplied"?{kind:"applied"}:outcome));}}/>:null}
    {change.isError?<p role="alert">{asAppError(change.error).message}</p>:null}
    {review.isError?<p role="alert">{asAppError(review.error).message}</p>:null}
    {completed&&workingId&&!version?<button className="secondary" disabled={busy} onClick={()=>review.mutate()}>查看待应用的修改</button>:null}
  </Sheet>;
}
