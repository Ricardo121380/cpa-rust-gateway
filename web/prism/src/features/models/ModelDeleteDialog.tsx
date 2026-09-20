import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { samePublicModel } from "./advancedRoutingModel";
import { isModelActionOwner, type DraftRoutingOwner } from "./advancedRoutingTask";
import { runModelTask, type ModelTaskReceipt } from "./modelTask";
import type { PublicModel } from "./model";

export function ModelDeleteDialog({model,owner,onClose,onDone,onReview}:Readonly<{model:PublicModel;owner:DraftRoutingOwner;onClose:()=>void;onDone:(receipt:ModelTaskReceipt)=>void;onReview:(receipt:ModelTaskReceipt)=>Promise<void>}>){
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[owner,onClose]);
  const remove=useMutation({mutationFn:async()=>{
    if(!isModelActionOwner(owner))throw new CancelledError({silent:true});
    const outcome=await runModelTask("移除公开模型",async(task)=>{
      const observed=await task.read<PublicModel>("getPublicModel",{path:{public_model_id:model.id}});
      if(!samePublicModel(model,observed))throw new Error("模型已变化，请重新读取后删除。");
      await task.mutate("deletePublicModel",{path:{public_model_id:model.id}});
    },{expectedSource:{id:owner.id,revision:owner.revision},probeUnchanged:true});
    return outcome.receipt;
  },onSuccess:next=>{if(isModelActionOwner(owner))setReceipt(next);},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const review=useMutation({mutationFn:async(next:ModelTaskReceipt)=>onReview(next)});
  const done=()=>{if(receipt?.kind==="unconfirmed")onClose();else if(receipt)onDone(receipt);else onClose();};
  const footer=receipt?.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(receipt)}>{review.isPending?"正在重读…":"核对配置"}</button></>:receipt?<SheetDismissButton onDismiss={done}>完成</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={remove.isPending}>取消</SheetDismissButton><button type="button" className="danger" disabled={remove.isPending||submitted.current} onClick={()=>{if(submitted.current)return;submitted.current=true;setError(undefined);remove.mutate();}}>确认删除</button></>;
  return <Sheet title={receipt?"模型删除结果":"删除公开模型"} description={receipt?"先核对工作配置，再继续其他操作。":"此操作会移除这个模型的别名、路由、候选和关联的访问组授权；其他模型、账号与密钥保留。"} layout="confirm" tone={receipt?"default":"danger"} onEscape={done} busy={remove.isPending||review.isPending} footer={footer}>
    {receipt?<><p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p><p>操作：删除公开模型 <strong className="mono">{model.model_name}</strong></p><p>工作配置 ID：<code className="mono">{receipt.workingVersion.id}</code></p>{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或稍后按此配置 ID 核对。</p>:null}</>:<p className="reveal-warning">删除 <strong className="mono">{model.model_name}</strong> 后，客户端将无法再用这个名称调用模型。</p>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
