import { CancelledError, isCancelledError, useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { samePublicModel } from "./advancedRoutingModel";
import { isModelActionOwner, type DraftRoutingOwner } from "./advancedRoutingTask";
import { runModelTask, type ModelTaskReceipt } from "./modelTask";
import type { AliasRecord, PublicModel, RoutingPage } from "./model";

type Phase={kind:"browse"}|{kind:"add";alias:string}|{kind:"delete";alias:string}|{kind:"receipt";receipt:ModelTaskReceipt;action:"add"|"delete";alias:string};
type AliasReader=(cursor?:string)=>Promise<RoutingPage<AliasRecord>>;

async function readAliases(read:AliasReader,versionId:string,revision:string):Promise<readonly AliasRecord[]> {
  const rows:AliasRecord[]=[];
  const seen=new Set<string>();
  let cursor:string|undefined;
  do{
    const page=await read(cursor);
    if(page.config_version!==versionId||page.revision!==revision)throw new Error("别名清单已变化，请重新读取。");
    rows.push(...page.items);
    if(rows.length>10000)throw new Error("别名超过本次读取范围。");
    cursor=page.next_cursor??undefined;
    if(cursor!==undefined){if(seen.has(cursor))throw new Error("别名分页未推进，请重新读取。");seen.add(cursor);}
  }while(cursor!==undefined);
  return rows;
}

export function ModelAliasDialog({model,owner,onClose,onDone,onReview}:Readonly<{model:PublicModel;owner:DraftRoutingOwner;onClose:()=>void;onDone:(receipt:ModelTaskReceipt)=>void;onReview:(receipt:ModelTaskReceipt)=>Promise<void>}>){
  const [phase,setPhase]=useState<Phase>({kind:"browse"});
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="model-alias-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[owner,onClose]);
  const aliases=useQuery({queryKey:["model-aliases",owner.id,owner.revision,model.id],retry:false,queryFn:()=>readAliases(cursor=>call<RoutingPage<AliasRecord>>("listModelAliases",{headers:{"X-Config-Version":owner.id},query:{limit:100,...(cursor?{cursor}:{})}}),owner.id,owner.revision)});
  const mutate=useMutation({mutationFn:async(action:{kind:"add"|"delete";alias:string})=>{
    if(!isModelActionOwner(owner))throw new CancelledError({silent:true});
    const result=await runModelTask(action.kind==="add"?"添加模型别名":"删除模型别名",async(task)=>{
      const observed=await task.read<PublicModel>("getPublicModel",{path:{public_model_id:model.id}});
      if(!samePublicModel(model,observed))throw new Error("模型已变化，请重新读取后维护别名。");
      const rows=await readAliases(cursor=>task.read<RoutingPage<AliasRecord>>("listModelAliases",{query:{limit:100,...(cursor?{cursor}:{})}}),task.version.id,task.revision());
      const existing=rows.find(row=>row.alias===action.alias);
      if(action.kind==="add"&&existing)throw new Error("该别名已存在，请使用原名称或重新选择。");
      if(action.kind==="delete"&&existing?.public_model_id!==model.id)throw new Error("别名归属已变化，请重新读取后操作。");
      await task.mutate(action.kind==="add"?"createModelAlias":"deleteModelAlias",{path:{public_model_id:model.id},body:{alias:action.alias}});
    },{expectedSource:{id:owner.id,revision:owner.revision},probeUnchanged:true});
    return result.receipt;
  },onSuccess:(receipt,action)=>{if(isModelActionOwner(owner))setPhase({kind:"receipt",receipt,action:action.kind,alias:action.alias});},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const review=useMutation({mutationFn:async(receipt:ModelTaskReceipt)=>onReview(receipt)});
  const done=()=>{if(phase.kind==="receipt"&&phase.receipt.kind!=="unconfirmed")onDone(phase.receipt);else onClose();};
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(phase.kind!=="add"||!phase.alias.trim()||submitted.current||!isModelActionOwner(owner))return;
    submitted.current=true;setError(undefined);mutate.mutate({kind:"add",alias:phase.alias});
  };
  const title=phase.kind==="add"?"添加模型别名":phase.kind==="delete"?"删除模型别名":phase.kind==="receipt"?"别名配置结果":`模型别名 · ${model.model_name}`;
  const footer=phase.kind==="browse"?<><SheetDismissButton className="secondary">关闭</SheetDismissButton><button type="button" onClick={()=>{setError(undefined);setPhase({kind:"add",alias:""});}}>添加别名</button></>:phase.kind==="add"?<><SheetDismissButton className="secondary" disabled={mutate.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={mutate.isPending||submitted.current||!phase.alias.trim()}>创建</button></>:phase.kind==="delete"?<><button type="button" className="secondary" disabled={mutate.isPending} onClick={()=>setPhase({kind:"browse"})}>返回</button><button type="button" className="danger" disabled={mutate.isPending||submitted.current} onClick={()=>{if(submitted.current)return;submitted.current=true;setError(undefined);mutate.mutate({kind:"delete",alias:phase.alias});}}>确认删除</button></>:phase.receipt.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(phase.receipt)}>{review.isPending?"正在重读…":"核对配置"}</button></>:<SheetDismissButton onDismiss={done}>完成</SheetDismissButton>;
  return <Sheet title={title} description={phase.kind==="browse"?"别名兼容旧客户端名称；原始模型 ID 不变。":phase.kind==="add"?"按原样填写客户端别名，不改写大小写或标点。":phase.kind==="delete"?"仅删除此名称；模型、路由、候选和授权保留。":"按实际保存与应用结果继续。"} layout={phase.kind==="delete"?"confirm":phase.kind==="add"?"form":"inspector"} tone={phase.kind==="delete"?"danger":"default"} onEscape={done} busy={mutate.isPending||review.isPending} isDirty={phase.kind==="add"&&phase.alias.length>0} footer={footer}>
    {phase.kind==="browse"?<>{aliases.isPending?<p role="status">正在读取别名…</p>:aliases.isError?<p role="alert">{asAppError(aliases.error).message} <button type="button" onClick={()=>void aliases.refetch()}>重新读取</button></p>:<>{aliases.data?.filter(row=>row.public_model_id===model.id).length?aliases.data?.filter(row=>row.public_model_id===model.id).map(row=><div className="data-toolbar" key={row.alias}><code className="mono">{row.alias}</code><button type="button" className="secondary" onClick={()=>{submitted.current=false;setError(undefined);setPhase({kind:"delete",alias:row.alias});}}>删除</button></div>):<p className="muted">没有别名；客户端使用 {model.model_name}。</p>}</>}</>:phase.kind==="add"?<form id={formId} className="sheet-form" onSubmit={submit}><fieldset disabled={mutate.isPending||submitted.current}><label>客户端别名<input className="mono" required maxLength={256} value={phase.alias} onChange={event=>setPhase({kind:"add",alias:event.target.value})}/></label></fieldset></form>:phase.kind==="delete"?<p className="reveal-warning">删除 <strong className="mono">{phase.alias}</strong> 后，客户端需改用 {model.model_name} 或其他已开放名称。</p>:<><p role={phase.receipt.kind==="unconfirmed"?"alert":"status"}>{phase.receipt.message}</p><p>操作：{phase.action==="add"?"添加":"删除"}别名 <strong className="mono">{phase.alias}</strong> · 模型 <strong className="mono">{model.model_name}</strong></p><p>工作配置 ID：<code className="mono">{phase.receipt.workingVersion.id}</code></p>{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或稍后按此配置 ID 核对。</p>:null}</>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
