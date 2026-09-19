import { useMutation, useQuery } from "@tanstack/react-query";
import { useId, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { ReadStatus } from "../../components/ReadStatus";
import { resourceName } from "../../utils/resourceNames";
import { protocolName } from "../accounts/presentation";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { readRoutingRows } from "./connectModel";
import { useModelConnections } from "./useModelConnections";
import { runModelTask, sameCandidate, type ModelTaskReceipt } from "./modelTask";
import { validCandidateParams, type CandidateRecord, type PublicModel, type RouteListItem } from "./model";

type Patch=Readonly<{enabled:boolean;priority:number;weight:number}>;
type Action=Readonly<{kind:"edit"|"remove";candidate:CandidateRecord;patch:Patch;sourceLabel:string;lastEnabled:boolean;siblings:readonly Readonly<{id:string;enabled:boolean}>[]}>;

function sameSiblingState(left:readonly Readonly<{id:string;enabled:boolean}>[],right:readonly CandidateRecord[]):boolean{
  return left.length===right.length&&left.every(row=>right.some(current=>current.id===row.id&&current.enabled===row.enabled));
}

export function ModelConnectionsDialog({model,onClose,onSaved,onAdd}:Readonly<{model:PublicModel;onClose:()=>void;onSaved:(v:ConfigVersionSummary)=>void;onAdd:()=>void}>) {
  const formId=useId();
  const scope=useVersionStore(s=>s.context?.configVersionId);
  const topology=useModelConnections();
  const providers=useQuery({queryKey:["upstreams",scope],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const route=topology.data?.routes.find(row=>row.public_model_id===model.id);
  const rows=topology.data?.candidates.filter(row=>row.route_id===route?.id)??[];
  const mappings=new Set(rows.map(row=>row.upstream_model));
  const [action,setAction]=useState<Action>();
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [validation,setValidation]=useState<string>();
  const submitted=useRef(false);
  const sourceName=(candidate:CandidateRecord)=>{
    const endpoint=topology.data?.endpoints.find(row=>row.id===candidate.endpoint_id);
    const provider=providers.data?.find(row=>row.id===endpoint?.upstream_id);
    return `${resourceName(endpoint?.upstream_id??"","upstream",provider?.name)} · ${protocolName(endpoint?.api_format??"")} · ${endpoint?new URL(endpoint.base_url).host:"接口未找到"}`;
  };
  const lastEnabled=action?.lastEnabled??false;
  const changed=action?.kind==="edit"&&(action.patch.enabled!==action.candidate.enabled||action.patch.priority!==action.candidate.priority||action.patch.weight!==action.candidate.weight);
  const save=useMutation({mutationFn:async(input:Action)=>runModelTask(`${input.kind==="remove"?"移除":"更新"}模型来源 · ${model.model_name}`,async(task)=>{
    const owner=(await readRoutingRows<RouteListItem>(task,"listRoutes")).find(row=>row.id===input.candidate.route_id);
    if(owner?.public_model_id!==model.id)throw new Error("来源连接已不属于所选模型，请重新读取。");
    const candidates=(await readRoutingRows<CandidateRecord>(task,"listRouteCandidates")).filter(row=>row.route_id===input.candidate.route_id);
    const current=candidates.find(row=>row.id===input.candidate.id);
    if(!sameCandidate(current,input.candidate))throw new Error("来源连接已变化，请关闭后重新读取。");
    if(!sameSiblingState(input.siblings,candidates.filter(row=>row.id!==input.candidate.id)))throw new Error("其他来源连接已变化，请重新确认影响。");
    if(input.lastEnabled!==(input.candidate.enabled&&!candidates.some(row=>row.id!==input.candidate.id&&row.enabled)))throw new Error("模型来源范围已变化，请重新确认影响。");
    if(input.kind==="edit"&&!validCandidateParams(input.patch.priority,input.patch.weight))throw new Error("优先级或权重超出允许范围。");
    if(input.lastEnabled&&(input.kind==="remove"||!input.patch.enabled)){
      const currentModel=await task.read<PublicModel>("getPublicModel",{path:{public_model_id:model.id}});
      if(currentModel.id!==model.id||currentModel.model_name!==model.model_name)throw new Error("模型已变化，请重新读取。");
      if(currentModel.status==="active")await task.mutate("updatePublicModel",{path:{public_model_id:model.id},body:{...currentModel,status:"disabled"}});
    }
    if(input.kind==="remove")await task.mutate("deleteRouteCandidate",{path:{route_id:input.candidate.route_id,candidate_id:input.candidate.id}});
    else {
      const {route_id,...body}=input.candidate;
      await task.mutate("updateRouteCandidate",{path:{route_id,candidate_id:input.candidate.id},body:{...body,...input.patch}});
    }
  }),onSuccess:({receipt:next})=>setReceipt(next),onError:()=>{submitted.current=false;}});
  const begin=(kind:Action["kind"],candidate:CandidateRecord)=>{setValidation(undefined);submitted.current=false;setAction({kind,candidate,patch:{enabled:candidate.enabled,priority:candidate.priority,weight:candidate.weight},sourceLabel:sourceName(candidate),lastEnabled:candidate.enabled&&!rows.some(row=>row.id!==candidate.id&&row.enabled),siblings:rows.filter(row=>row.id!==candidate.id).map(row=>({id:row.id,enabled:row.enabled}))});};
  const submit=(event:FormEvent)=>{event.preventDefault();if(!action||submitted.current)return;if(action.kind==="edit"&&!validCandidateParams(action.patch.priority,action.patch.weight)){setValidation("优先级须为非负整数，权重须为 1–10000。");return;}submitted.current=true;save.mutate(action);};
  const confirm=()=>{if(!action||submitted.current)return;submitted.current=true;save.mutate(action);};
  const done=()=>{if(receipt&&receipt.kind!=="unchanged")onSaved(receipt.workingVersion);else onClose();};
  const footer=receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="saved_applied"||receipt.kind==="unchanged"?"完成":"核对配置"}</SheetDismissButton>:action?.kind==="edit"?<><SheetDismissButton className="secondary" disabled={save.isPending} onDismiss={()=>setAction(undefined)}>返回来源</SheetDismissButton><button type="submit" form={formId} disabled={save.isPending||submitted.current||!changed}>保存并应用</button></>:action?.kind==="remove"?<><SheetDismissButton className="secondary" disabled={save.isPending} onDismiss={()=>setAction(undefined)}>返回来源</SheetDismissButton><button type="button" className="danger" disabled={save.isPending||submitted.current} onClick={confirm}>确认移除</button></>:<><SheetDismissButton className="secondary">关闭</SheetDismissButton><button type="button" disabled={topology.isPending||topology.isError||mappings.size>1} onClick={onAdd}>添加来源连接</button></>;
  return <Sheet title={receipt?"模型来源结果":action?.kind==="remove"?"移除模型来源":action?.kind==="edit"?"编辑模型来源":`${model.model_name} · 来源连接`} description={receipt?"请核对本次写入与应用状态。":action?"操作只影响选中的这一条来源路径。":"查看此模型的上游来源，并选择要维护的路径。"} layout={!action&&!receipt?"inspector":action?.kind==="remove"?"confirm":"form"} tone={action?.kind==="remove"?"danger":"default"} onEscape={()=>!save.isPending&&(action&&!receipt?setAction(undefined):done())} busy={save.isPending} isDirty={!!changed} footer={footer}>
    {receipt?<div role="status"><p>{receipt.message}</p><p>已确认保存 {receipt.acknowledgedWrites} 步。</p></div>:action?.kind==="edit"?<form id={formId} className="sheet-form" onSubmit={submit}>
      <p><strong>{action.sourceLabel}</strong></p><p className="entity-meta">上游模型：<code>{action.candidate.upstream_model}</code> · {action.candidate.transform_mode}</p>
      <label className="check-row"><input type="checkbox" checked={action.patch.enabled} disabled={save.isPending||submitted.current} onChange={event=>setAction({...action,patch:{...action.patch,enabled:event.target.checked}})}/>启用此来源</label>
      <div className="model-source-controls"><label>优先级<input type="number" min={0} required value={action.patch.priority} disabled={save.isPending||submitted.current} onChange={event=>setAction({...action,patch:{...action.patch,priority:Number(event.target.value)}})}/></label><label>权重<input type="number" min={1} max={10000} required value={action.patch.weight} disabled={save.isPending||submitted.current} onChange={event=>setAction({...action,patch:{...action.patch,weight:Number(event.target.value)}})}/></label></div>
      {lastEnabled&&!action.patch.enabled?<p className="reveal-warning">这是最后一个启用来源，保存后此公开模型也会停用。</p>:null}
      {validation?<p role="alert">{validation}</p>:null}{save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
    </form>:action?.kind==="remove"?<><p><strong>{action.sourceLabel}</strong></p><p>上游模型 <code>{action.candidate.upstream_model}</code> · 转换路径 {action.candidate.transform_mode}</p><p>此操作移除这一条模型来源，保留账号与历史记录。{lastEnabled?"这是最后一个启用来源，公开模型也会停用。":""}</p>{save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}</>:<>
      <ReadStatus pending={topology.isPending} error={topology.error??providers.error} hasData={!!topology.data} retry={()=>void topology.refetch()}/>
      <div className="model-source-list">{rows.map(candidate=><section className="model-source-card" key={candidate.id}><h4>{sourceName(candidate)}</h4><p className="entity-meta">上游模型 <code>{candidate.upstream_model}</code> · {candidate.transform_mode} · {candidate.enabled?"已启用":"已停用"}</p><div className="page-actions"><button type="button" className="secondary" onClick={()=>begin("edit",candidate)}>编辑路径</button><button type="button" className="danger" onClick={()=>begin("remove",candidate)}>移除路径</button></div></section>)}</div>
      {!rows.length&&!topology.isPending?<p className="empty-state">尚未连接提供商。</p>:null}
      {mappings.size>1?<p className="muted">此模型有多种上游映射。添加路径请使用高级路由配置，避免自动选择错误的上游模型。</p>:null}
    </>}
  </Sheet>;
}
