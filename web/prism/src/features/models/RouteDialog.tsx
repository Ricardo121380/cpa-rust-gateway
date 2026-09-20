import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { sameRoute } from "./advancedRoutingModel";
import { isModelActionOwner, readRoutingInventory, runDraftRoutingWrite, type DraftRoutingOwner } from "./advancedRoutingTask";
import type { ModelTaskReceipt } from "./modelTask";
import { ROUTE_POLICY, validRouteParams, type RouteListItem } from "./model";

export type RouteAction=Readonly<{kind:"edit"|"delete";owner:DraftRoutingOwner;route:RouteListItem}>;

export function RouteDialog({action,onClose,onDone}:Readonly<{action:RouteAction;onClose:()=>void;onDone:(receipt:ModelTaskReceipt,action:RouteAction)=>void}>){
  const [attempts,setAttempts]=useState(String(action.route.max_attempts));
  const [timeout,setTimeoutValue]=useState(String(action.route.bootstrap_timeout_ms));
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="route-action-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(action.owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[action.owner,onClose]);
  const write=useMutation({mutationFn:async(body:{attempts:number;timeout:number}|undefined)=>{
    if(!isModelActionOwner(action.owner))throw new CancelledError({silent:true});
    const result=await runDraftRoutingWrite(action.owner,action.kind==="edit"?"updateRoute":"deleteRoute",{path:{route_id:action.route.id},...(body?{body:{id:action.route.id,policy:ROUTE_POLICY,max_attempts:body.attempts,bootstrap_timeout_ms:body.timeout}}:{})},async(read)=>{
      const routes=await readRoutingInventory<RouteListItem>(read,"listRoutes",action.owner);
      if(!sameRoute(routes.find(route=>route.id===action.route.id),action.route))throw new Error("路由或所属模型已变化，请重新读取后操作。");
    });
    return result.receipt;
  },onSuccess:next=>{if(isModelActionOwner(action.owner))setReceipt(next);},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(action.owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(submitted.current||!isModelActionOwner(action.owner))return;
    if(action.kind==="delete"){submitted.current=true;write.mutate(undefined);return;}
    const max=Number(attempts),budget=Number(timeout);
    if(!validRouteParams(max,budget)){setError("最大尝试次数须为 1–16，启动预算须为 1–120000 毫秒。");return;}
    submitted.current=true;setError(undefined);write.mutate({attempts:max,timeout:budget});
  };
  const done=()=>{if(receipt)onDone(receipt,action);else onClose();};
  const footer=receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="unconfirmed"?"核对草稿":"完成"}</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={write.isPending}>取消</SheetDismissButton><button type={action.kind==="delete"?"button":"submit"} form={action.kind==="delete"?undefined:formId} className={action.kind==="delete"?"danger":undefined} disabled={write.isPending||submitted.current} onClick={action.kind==="delete"?()=>{if(!submitted.current){submitted.current=true;write.mutate(undefined);}}:undefined}>{action.kind==="delete"?"确认删除":"保存路由"}</button></>;
  return <Sheet title={receipt?"路由配置结果":action.kind==="delete"?"删除路由":"编辑路由"} description={receipt?"此操作只保存到当前草稿；发布需另行完成。":action.kind==="delete"?"路由删除会一并移除候选与访问组授权；公开模型和账号保留。":"调整重试次数与启动预算，不会改变候选或授权。"} layout={receipt?"inspector":action.kind==="delete"?"confirm":"form"} tone={action.kind==="delete"&&!receipt?"danger":"default"} onEscape={done} busy={write.isPending} isDirty={!receipt&&action.kind==="edit"&&(attempts!==String(action.route.max_attempts)||timeout!==String(action.route.bootstrap_timeout_ms))} footer={footer}>
    {receipt?<p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p>:action.kind==="delete"?<p className="reveal-warning">删除路由 <strong className="mono">{action.route.id}</strong> 后，所属公开模型将暂时没有可用路径，直到重新配置路由。</p>:<form id={formId} className="sheet-form" onSubmit={submit}><fieldset disabled={write.isPending||submitted.current}>
      <p className="muted">所属模型与路由标识保持不变。</p>
      <label>最大尝试次数<input type="number" min={1} max={16} value={attempts} onChange={event=>setAttempts(event.target.value)}/></label>
      <label>启动预算（毫秒）<input type="number" min={1} max={120000} value={timeout} onChange={event=>setTimeoutValue(event.target.value)}/></label>
    </fieldset></form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
