import { useMutation, useQuery } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { Sheet } from "../../components/Sheet";
import { ReadStatus } from "../../components/ReadStatus";
import { resourceName } from "../../utils/resourceNames";
import { protocolName } from "../accounts/presentation";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { readRoutingRows } from "./connectModel";
import { useModelConnections } from "./useModelConnections";
import type { CandidateRecord, PublicModel } from "./model";

export function ModelConnectionsDialog({model,onClose,onSaved,onAdd}:Readonly<{model:PublicModel;onClose:()=>void;onSaved:(v:ConfigVersionSummary)=>void;onAdd:()=>void}>) {
  const scope=useVersionStore(s=>s.context?.configVersionId);
  const topology=useModelConnections();
  const providers=useQuery({queryKey:["upstreams",scope],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const route=topology.data?.routes.find(r=>r.public_model_id===model.id);
  const rows=topology.data?.candidates.filter(c=>c.route_id===route?.id)??[];
  const groups=new Map<string,CandidateRecord[]>();
  for(const row of rows){const key=JSON.stringify([row.endpoint_id,row.upstream_model]);groups.set(key,[...(groups.get(key)??[]),row]);}
  const [workingId,setWorkingId]=useState<string>();
  const [removing,setRemoving]=useState<CandidateRecord>();
  const save=useMutation({mutationFn:async({candidate,patch,remove}:{candidate:CandidateRecord;patch?:{enabled:boolean;weight:number;priority:number};remove?:boolean})=>{
    const task=await beginConfigurationTask(`${remove?"移除":"更新"}模型连接 · ${model.model_name}`);setWorkingId(task.version.id);
    const current=(await readRoutingRows<CandidateRecord>(task,"listRouteCandidates")).filter(c=>c.route_id===candidate.route_id);
    const original=current.find(c=>c.id===candidate.id);
    if(JSON.stringify(original)!==JSON.stringify(candidate))throw new Error("连接已被修改，请关闭后重新读取。");
    if((remove||patch?.enabled===false)&&!current.some(c=>c.id!==candidate.id&&c.enabled)) {
      const currentModel=await task.read<PublicModel>("getPublicModel",{path:{public_model_id:model.id}});
      if(currentModel.status==="active")await task.mutate("updatePublicModel",{path:{public_model_id:model.id},body:{...currentModel,status:"disabled"}});
    }
    if(remove)await task.mutate("deleteRouteCandidate",{path:{route_id:candidate.route_id,candidate_id:candidate.id}});
    else {
      const {route_id,...body}=candidate;
      await task.mutate("updateRouteCandidate",{path:{route_id,candidate_id:candidate.id},body:{...body,...patch}});
    }
    return task.finish();
  },onSuccess:onSaved});
  const submit=(event:FormEvent<HTMLFormElement>,candidate:CandidateRecord)=>{event.preventDefault();const data=new FormData(event.currentTarget);save.mutate({candidate,patch:{enabled:data.get("enabled")==="on",priority:Number(data.get("priority")),weight:Number(data.get("weight"))}});};
  return <Sheet title={`${model.model_name} · 来源连接`} layout="inspector" onEscape={()=>!save.isPending&&onClose()}>
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
    <ReadStatus pending={topology.isPending} error={topology.error??providers.error} hasData={!!topology.data} retry={()=>void topology.refetch()}/>
    <div className="model-source-list">{[...groups.entries()].map(([key,candidates])=>{
      const candidate=candidates[0]!;
      const endpoint=topology.data?.endpoints.find(e=>e.id===candidate.endpoint_id);
      const controls=candidates.map(c=><form key={c.id} className="sheet-form" onSubmit={event=>submit(event,c)}>
        {candidates.length>1?<h5>{({canonical:"标准协议",canonical_bridge:"兼容协议转换",lossless_bridge:"无损协议转换",passthrough:"原始协议转发"} as Record<string,string>)[c.transform_mode]}</h5>:null}
        <label className="check-row"><input type="checkbox" name="enabled" defaultChecked={c.enabled} disabled={save.isPending}/>启用{candidates.length>1?"此路径":"此连接"}</label>
        <div className="model-source-controls"><label>优先级<input type="number" name="priority" min={0} defaultValue={c.priority} required disabled={save.isPending}/></label><label>权重<input type="number" name="weight" min={1} max={10000} required defaultValue={c.weight} disabled={save.isPending}/></label></div>
        <div className="sheet-actions"><button type="button" className="danger" disabled={save.isPending} onClick={()=>setRemoving(c)}>移除{candidates.length>1?"此路径":"连接"}</button><button disabled={save.isPending||topology.isError}>保存并应用</button></div>
      </form>);
      return <section className="model-source-card" key={key}><h4>{resourceName(endpoint?.upstream_id??"","upstream",providers.data?.find(p=>p.id===endpoint?.upstream_id)?.name)}</h4>
        <p className="entity-meta">{protocolName(endpoint?.api_format??"")} · {endpoint?new URL(endpoint.base_url).host:"接口未找到"}</p>
        {candidate.upstream_model!==model.model_name?<p>上游模型 <code>{candidate.upstream_model}</code></p>:null}
        {candidates.length>1?<details><summary>高级转换路径 · {candidates.length}</summary>{controls}</details>:controls}
      </section>;
    })}</div>
    {!rows.length&&!topology.isPending?<p className="empty-state">尚未连接提供商。</p>:null}
    <p className="stat-sub">停用或移除最后一个启用连接，会同时停用此模型。账号与历史记录保留。</p>
    {removing?<div className="action-notice" role="alert"><p>确认移除所选连接？</p><div className="sheet-actions"><button className="secondary" disabled={save.isPending} onClick={()=>setRemoving(undefined)}>取消</button><button className="danger" disabled={save.isPending} onClick={()=>save.mutate({candidate:removing,remove:true})}>确认移除</button></div></div>:null}
    <div className="sheet-actions"><button className="secondary" disabled={save.isPending} onClick={onClose}>关闭</button><button disabled={save.isPending} onClick={onAdd}>添加来源连接</button></div>
  </Sheet>;
}
