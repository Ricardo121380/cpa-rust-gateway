import { useMutation, isCancelledError, CancelledError } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import type { NativeReceipt } from "./NativeAccountDialog";
import { RuntimeApplyNotice } from "./RuntimeApplyNotice";

export type AccountTarget=Readonly<{id:string;native:boolean;name:string;provider:string;enabled:boolean;revision:number}>;
export type AccountAction="enable"|"disable"|"remove";
const labels={enable:"启用",disable:"停用",remove:"移除"};
export function AccountBatchDialog({targets,action,onClose,onCompleted}:Readonly<{targets:readonly AccountTarget[];action:AccountAction;onClose:()=>void;onCompleted:(notice:string,version?:ConfigVersionSummary)=>void}>) {
  const [results,setResults]=useState<string[]>(targets.map(()=>"待处理"));
  const [completed,setCompleted]=useState(false);
  const [error,setError]=useState<string>();
  const [needsApply,setNeedsApply]=useState(false);
  const [version,setVersion]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const change=useMutation({mutationFn:async()=>{
    if(targets.length<1||targets.length>20)throw new Error("一次最多处理 20 份授权。");
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    const needsChange=(target:AccountTarget)=>action==="remove"||target.enabled!==(action==="enable");
    const task=targets.some((target)=>!target.native&&needsChange(target))?await beginConfigurationTask(`${labels[action]}账号`):undefined;
    if(task)setWorkingId(task.version.id);
    const states=targets.map(()=>"未执行");let ordinarySaved=false;let canApply=true;
    for(const [index,target] of targets.entries()) {
      assertOwner();
      if(!needsChange(target)){states[index]="无需更改";continue;}
      try {
        if(target.native) {
          const path={account_id:target.id};
          const receipt=action==="remove"?await call<NativeReceipt>("deleteNativeAccount",{path,query:{revision:target.revision}}):await call<NativeReceipt>("updateNativeAccount",{path,body:{revision:target.revision,enabled:action==="enable"}});
          states[index]=receipt.runtime_applied?"已应用":"已保存，待应用";
          if(!receipt.runtime_applied){setNeedsApply(true);canApply=false;setResults([...states]);break;}
        } else {
          const current=await task!.read<{revision:number}>("getCredential",{path:{credential_id:target.id}});
          if(current.revision!==target.revision)throw new Error("账号授权已变化，请重新选择后核对。");
          if(action==="remove")await task!.mutate("deleteCredential",{path:{credential_id:target.id}});
          else await task!.mutate("updateCredentialStatus",{path:{credential_id:target.id},body:{status:action==="enable"?"active":"disabled",credential_revision:target.revision}});
          ordinarySaved=true;states[index]="已保存，待应用";
        }
      } catch(cause) {
        if(isCancelledError(cause))throw cause;
        states[index]=asAppError(cause).message;canApply=false;setResults([...states]);break;
      }
      setResults([...states]);
    }
    if(task&&ordinarySaved&&canApply) {
      try {
        const finished=await task.finish();setVersion(finished);
        if(finished.status==="active")for(const [index,target] of targets.entries())if(!target.native&&states[index]==="已保存，待应用")states[index]="已应用";
      } catch(cause){if(isCancelledError(cause))throw cause;setError(asAppError(cause).message);}
    }
    return states;
  },onSuccess:(states)=>{setResults(states);setCompleted(true);},onError:(cause)=>{if(!isCancelledError(cause))setError(asAppError(cause).message);}});
  const review=useMutation({mutationFn:()=>call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}}),onSuccess:(version)=>onCompleted("请核对已保存的账号修改。",version)});
  const busy=change.isPending||review.isPending;
  const close=()=>{if(busy)return;if(completed)onCompleted(needsApply?"账号修改已保存，运行配置暂未应用。":"账号操作已处理，请查看各项结果。",version);else onClose();};
  return <Sheet title={`${labels[action]} ${targets.length} 份授权`} onEscape={close}>
    {action==="remove"?<p>移除选中的授权及其连接，历史请求与费用保留。</p>:<p>仅更改下列授权的启停状态。</p>}
    <div className="tablewrap"><table><thead><tr><th>账号</th><th>渠道</th><th>结果</th></tr></thead><tbody>{targets.map((target,index)=><tr key={`${target.native}:${target.id}`}><td>{target.name}</td><td>{target.provider}</td><td>{results[index]}</td></tr>)}</tbody></table></div>
    {needsApply?<RuntimeApplyNotice onApplied={()=>{setNeedsApply(false);setResults((items)=>items.map((value,index)=>targets[index]?.native&&value==="已保存，待应用"?"已应用":value));}}/>:null}
    {error?<p role="alert">{error}</p>:null}
    {review.isError?<p role="alert">{asAppError(review.error).message}</p>:null}
    {completed&&workingId&&!version?<button className="secondary" disabled={busy} onClick={()=>review.mutate()}>查看待应用的修改</button>:null}
    <div className="sheet-actions"><button className="secondary" disabled={busy} onClick={close}>{completed?"完成":"取消"}</button>{!completed?<button disabled={busy} onClick={()=>change.mutate()}>确认{labels[action]}</button>:null}</div>
  </Sheet>;
}
