import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { protocolName } from "../accounts/presentation";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel } from "./connectModel";

export function ConnectModelDialog({seed,onClose,onSaved}:Readonly<{seed?:{model:string;endpoint:string};onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const scope=useVersionStore((state)=>state.context?.configVersionId);
  const endpoints=useManagedInventory("endpoints");
  const providers=useQuery({queryKey:["upstreams",scope],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const rows=endpoints.data?.pages.flatMap((page)=>page.items)??[];
  const [model,setModel]=useState(seed?.model??"");
  const [upstreamModel,setUpstreamModel]=useState(seed?.model??"");
  const [endpoint,setEndpoint]=useState(seed?.endpoint??"");
  const [manual,setManual]=useState(false);
  const [workingId,setWorkingId]=useState<string>();
  const submitted=useRef(false);
  const save=useMutation({mutationFn:async()=>{
    const task=await beginConfigurationTask(`开放模型 · ${model.trim()}`);setWorkingId(task.version.id);
    await connectModel(task,{model:model.trim(),upstreamModel:upstreamModel.trim(),endpointId:endpoint,allowUnlisted:manual});
    return task.finish();
  },onSuccess:onSaved});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  return <Sheet title="开放模型" onEscape={()=>!save.isPending&&onClose()}><form className="sheet-form" onSubmit={submit}>
    <label>客户端模型名称<input required maxLength={256} value={model} onChange={(event)=>setModel(event.target.value)} disabled={submitted.current} placeholder="客户端请求中填写的 model"/></label>
    <label>提供商模型 ID<input required maxLength={256} value={upstreamModel} onChange={(event)=>setUpstreamModel(event.target.value)} disabled={submitted.current} placeholder="提供商支持的完整模型 ID"/></label>
    <label>接口连接<select required value={endpoint} onChange={(event)=>setEndpoint(event.target.value)} disabled={submitted.current}><option value="">选择提供商的接口</option>{rows.map((row)=><option key={row.id} value={row.id}>{resourceName(row.upstream_id,"upstream",providers.data?.find((provider)=>provider.id===row.upstream_id)?.name)} · {protocolName(row.api_format)} · {new URL(row.base_url).host}{row.enabled?"":" · 已停用"}</option>)}</select></label>
    {endpoints.isError||providers.isError?<p role="alert">{asAppError(endpoints.error??providers.error).message}</p>:null}
    {endpoints.hasNextPage?<button type="button" className="secondary" disabled={endpoints.isFetching||submitted.current} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}
    {!rows.length&&!endpoints.isFetching?<Link to="/upstreams?add=provider" onClick={onClose}>先添加提供商</Link>:null}
    <label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置目录未收录的模型；已确认账号可使用</label>
    <p className="muted">保存后可在客户端密钥中选择此模型。</p>
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
    <div className="sheet-actions"><button type="button" className="secondary" disabled={save.isPending} onClick={onClose}>取消</button><button type="submit" disabled={submitted.current||!scope}>{save.isPending?"正在保存…":"保存并应用"}</button></div>
  </form></Sheet>;
}
