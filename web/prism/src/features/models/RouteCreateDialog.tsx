import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { samePublicModel } from "./advancedRoutingModel";
import { isModelActionOwner, readRoutingInventory, runDraftRoutingWrite, type DraftRoutingOwner } from "./advancedRoutingTask";
import { type ModelTaskReceipt } from "./modelTask";
import { ROUTE_POLICY, validRouteParams, type PublicModel, type RouteListItem } from "./model";

type Input={id:string;attempts:string;timeout:string};
type Result={receipt:ModelTaskReceipt;routeId?:string;requestedRouteId:string};

export function RouteCreateDialog({model,owner,onClose,onDone,onReview}:Readonly<{model:PublicModel;owner:DraftRoutingOwner;onClose:()=>void;onDone:(result:Result)=>void;onReview:(receipt:ModelTaskReceipt)=>Promise<void>}>){
  const [form,setForm]=useState<Input>({id:"",attempts:"3",timeout:"30000"});
  const [result,setResult]=useState<Result>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="create-route-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[owner,onClose]);
  const save=useMutation({mutationFn:async(input:Readonly<{id:string;attempts:number;timeout:number}>)=>{
    if(!isModelActionOwner(owner))throw new CancelledError({silent:true});
    const outcome=await runDraftRoutingWrite<{id:string}>(owner,"createRoute",{path:{public_model_id:model.id},body:{id:input.id,policy:ROUTE_POLICY,max_attempts:input.attempts,bootstrap_timeout_ms:input.timeout}},async(read)=>{
      const current=await read<PublicModel>("getPublicModel",{path:{public_model_id:model.id}});
      if(!samePublicModel(model,current))throw new Error("模型已变化，请重新读取后配置路由。");
      const routes=await readRoutingInventory<RouteListItem>(read,"listRoutes",owner);
      if(routes.some(route=>route.id===input.id||route.public_model_id===model.id))throw new Error("模型已有路由或路由标识已被使用，请重新核对。");
    });
    return {receipt:outcome.receipt,routeId:outcome.value?.id,requestedRouteId:input.id};
  },onSuccess:next=>{if(isModelActionOwner(owner))setResult(next);},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const review=useMutation({mutationFn:async(receipt:ModelTaskReceipt)=>onReview(receipt)});
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(submitted.current||!form.id.trim()||!isModelActionOwner(owner))return;
    const attempts=Number(form.attempts),timeout=Number(form.timeout);
    if(!validRouteParams(attempts,timeout)){setError("重试次数须为 1–16，启动预算须为 1–120000 毫秒。");return;}
    submitted.current=true;setError(undefined);save.mutate({id:form.id.trim(),attempts,timeout});
  };
  const done=()=>{if(result?.receipt.kind==="unconfirmed")onClose();else if(result)onDone(result);else onClose();};
  const footer=result?.receipt.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(result.receipt)}>{review.isPending?"正在重读…":"核对草稿"}</button></>:result?<SheetDismissButton onDismiss={done}>继续配置候选</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={save.isPending||submitted.current||!form.id.trim()}>创建路由</button></>;
  return <Sheet title={result?"路由配置结果":`为 ${model.model_name} 创建路由`} description={result?"路由和候选是两步草稿操作；创建本身不会发布。":"路由定义重试与启动预算，创建后还需添加至少一个可用候选。"} layout={result?"inspector":"form"} onEscape={done} busy={save.isPending||review.isPending} isDirty={!result&&(!!form.id||form.attempts!=="3"||form.timeout!=="30000")} footer={footer}>
    {result?<><p role={result.receipt.kind==="unconfirmed"?"alert":"status"}>{result.receipt.message}</p><p>目标路由：<strong className="mono">{result.requestedRouteId}</strong> · 模型 <strong className="mono">{model.model_name}</strong></p><p>工作配置 ID：<code className="mono">{result.receipt.workingVersion.id}</code></p>{result.routeId?<p>路由已创建；请在下方工作台添加候选后再校验。</p>:null}{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或稍后按此配置 ID 核对；重读不代表原请求已确认成功。</p>:null}</>:<form id={formId} className="sheet-form" onSubmit={submit}><fieldset disabled={save.isPending||submitted.current}>
      <label>路由标识<input className="mono" required maxLength={128} value={form.id} onChange={event=>setForm({...form,id:event.target.value})}/></label>
      <label>最大尝试次数<input type="number" min={1} max={16} value={form.attempts} onChange={event=>setForm({...form,attempts:event.target.value})}/></label>
      <label>启动预算（毫秒）<input type="number" min={1} max={120000} value={form.timeout} onChange={event=>setForm({...form,timeout:event.target.value})}/></label>
    </fieldset></form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
