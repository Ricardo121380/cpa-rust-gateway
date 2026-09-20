import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { captureModelActionOwner, isModelActionOwner, type DraftRoutingOwner } from "./advancedRoutingTask";
import { samePublicModel } from "./advancedRoutingModel";
import { runModelTask, type ModelTaskReceipt } from "./modelTask";
import { SEMANTIC_CAPABILITIES, toggleCapability, type PublicModel } from "./model";

type EditableModel={id:string;model_name:string;status:"active"|"disabled";display_name:string;capabilities:Record<string,boolean>};
const blank=():EditableModel=>({id:"",model_name:"",status:"active",display_name:"",capabilities:{streaming:true}});
const copy=(model:PublicModel):EditableModel=>({...model,capabilities:{...model.capabilities}});

export function ModelEditorDialog({initial,owner,onClose,onDone,onReview}:Readonly<{initial?:PublicModel;owner:DraftRoutingOwner;onClose:()=>void;onDone:(receipt:ModelTaskReceipt)=>void;onReview:(receipt:ModelTaskReceipt)=>Promise<void>}>){
  const [form,setForm]=useState<EditableModel>(()=>initial?copy(initial):blank());
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="advanced-model-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[owner,onClose]);
  const save=useMutation({mutationFn:async(desired:PublicModel)=>{
    if(!isModelActionOwner(owner))throw new CancelledError({silent:true});
    const outcome=await runModelTask(initial?"编辑公开模型":"新增公开模型",async(task)=>{
      if(initial){
        const observed=await task.read<PublicModel>("getPublicModel",{path:{public_model_id:initial.id}});
        if(!samePublicModel(initial,observed))throw new Error("模型已变化，请重新读取后编辑。");
        if(samePublicModel(observed,desired))return;
      }else{
        const models=await task.read<PublicModel[]>("listPublicModels");
        if(models.some(model=>model.id===desired.id||model.model_name===desired.model_name))throw new Error("模型 ID 或客户端模型名已存在，请重新核对。");
      }
      await task.mutate(initial?"updatePublicModel":"createPublicModel",{
        ...(initial?{path:{public_model_id:initial.id}}:{}),body:desired,
      });
    },{expectedSource:{id:owner.id,revision:owner.revision},probeUnchanged:true});
    return outcome.receipt;
  },onSuccess:next=>{if(isModelActionOwner(owner))setReceipt(next);},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const review=useMutation({mutationFn:async(next:ModelTaskReceipt)=>onReview(next)});
  const desired:PublicModel={...form,capabilities:{...form.capabilities}};
  const dirty=receipt===undefined&&(initial?!samePublicModel(initial,desired):JSON.stringify(form)!==JSON.stringify(blank()));
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(submitted.current||!form.id.trim()||!form.model_name.trim()||!isModelActionOwner(owner))return;
    submitted.current=true;
    setError(undefined);
    save.mutate(desired);
  };
  const done=()=>{if(receipt?.kind==="unconfirmed")onClose();else if(receipt)onDone(receipt);else onClose();};
  const footer=receipt?.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(receipt)}>{review.isPending?"正在重读…":"核对配置"}</button></>:receipt?<SheetDismissButton onDismiss={done}>完成</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={save.isPending||submitted.current||!form.id.trim()||!form.model_name.trim()}>保存</button></>;
  return <Sheet title={receipt?"模型配置结果":initial?`编辑 ${initial.model_name}`:"新建公开模型"} description={receipt?"按当前草稿或活动配置的实际保存结果继续。":"客户端使用精确模型 ID；显示名仅用于管理端阅读。"} layout={receipt?"inspector":"form"} onEscape={done} busy={save.isPending||review.isPending} isDirty={dirty} footer={footer}>
    {receipt?<><p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p><p>操作：{initial?"编辑":"新增"}公开模型 <strong className="mono">{initial?.model_name??form.model_name}</strong></p><p>工作配置 ID：<code className="mono">{receipt.workingVersion.id}</code></p><p className="muted">{receipt.workingVersion.description}</p>{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或稍后按此配置 ID 核对。</p>:null}</>:<form id={formId} className="sheet-form" onSubmit={submit}>
      <fieldset disabled={save.isPending||submitted.current}>
        {initial?null:<label>模型 ID<input className="mono" required maxLength={128} value={form.id} onChange={event=>setForm({...form,id:event.target.value})}/></label>}
        <label>客户端模型名<input className="mono" required maxLength={256} value={form.model_name} onChange={event=>setForm({...form,model_name:event.target.value})}/></label>
        <label>管理端显示名<input maxLength={256} value={form.display_name} onChange={event=>setForm({...form,display_name:event.target.value})}/></label>
        <label className="toggle-row"><input type="checkbox" checked={form.status==="active"} onChange={event=>setForm({...form,status:event.target.checked?"active":"disabled"})}/>启用模型</label>
        <fieldset className="capability-set"><legend>所需能力</legend>{SEMANTIC_CAPABILITIES.map(capability=><label key={capability} className="toggle-row"><input type="checkbox" checked={form.capabilities[capability]===true} onChange={event=>setForm({...form,capabilities:toggleCapability(form.capabilities,capability,event.target.checked)})}/><span>{capability}</span>{capability==="parallel_tools"?<small>同时需要 tools</small>:null}</label>)}</fieldset>
      </fieldset>
    </form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}

/** Capture from the selected configuration when an editor is opened, not when Save is clicked. */
export { captureModelActionOwner };
