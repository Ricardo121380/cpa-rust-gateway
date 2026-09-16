import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { protocolName } from "../accounts/presentation";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel, parseModelIds } from "./connectModel";

export function ConnectModelDialog({seed,endpointSeed,targetModelId,onClose,onSaved}:Readonly<{seed?:{model:string;endpoint:string;alias?:string};endpointSeed?:string;targetModelId?:string;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const formId="connect-model-form";
  const scope=useVersionStore((state)=>state.context?.configVersionId);
  const endpoints=useManagedInventory("endpoints");
  const providers=useQuery({queryKey:["upstreams",scope],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const rows=endpoints.data?.pages.flatMap((page)=>page.items)??[];
  const [alias,setAlias]=useState(seed?.alias??"");
  const [customAlias,setCustomAlias]=useState(!!seed?.alias);
  const [upstreamModel,setUpstreamModel]=useState(seed?.model??"");
  const [endpoint,setEndpoint]=useState(seed?.endpoint??endpointSeed??"");
  const [manual,setManual]=useState(false);
  const [workingId,setWorkingId]=useState<string>();
  const submitted=useRef(false);
  const save=useMutation({mutationFn:async()=>{
    const names=parseModelIds(upstreamModel);
    if(customAlias && (names.length!==1 || !alias.trim()))throw new Error("自定义别名仅适用于单个模型。");
    const task=await beginConfigurationTask(`接入模型 · ${names.length} 个`);setWorkingId(task.version.id);
    for(const model of names)await connectModel(task,{upstreamModel:model,alias:customAlias?alias.trim():undefined,targetModelId,endpointId:endpoint,allowUnlisted:manual});
    return task.finish();
  },onSuccess:onSaved});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  return <Sheet title="接入模型" description="使用上游返回的完整模型 ID，并明确选择它应连接的接口。" onEscape={()=>!save.isPending&&onClose()} busy={save.isPending} footer={<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={submitted.current||!scope}>{save.isPending?"正在保存…":"保存并应用"}</button></>}><form id={formId} className="sheet-form" onSubmit={submit}>
    <label>上游模型 ID<textarea required rows={4} maxLength={5140} value={upstreamModel} readOnly={!!targetModelId} onChange={(event)=>setUpstreamModel(event.target.value)} disabled={submitted.current} placeholder="每行一个完整模型 ID，最多 20 个"/></label>
    <p className="stat-sub">客户端直接使用这些模型 ID。同名模型会添加来源连接。</p>
    <details className="model-advanced" open={seed?.alias?true:undefined}><summary>自定义模型别名</summary><label className="check-row"><input type="checkbox" checked={customAlias} onChange={(event)=>setCustomAlias(event.target.checked)} disabled={submitted.current}/>使用不同的客户端名称</label>{customAlias?<label>别名<input required maxLength={256} value={alias} onChange={(event)=>setAlias(event.target.value)} disabled={submitted.current}/></label>:null}</details>
    <label>接口连接<select required value={endpoint} onChange={(event)=>setEndpoint(event.target.value)} disabled={submitted.current}><option value="">选择提供商的接口</option>{rows.map((row)=><option key={row.id} value={row.id}>{resourceName(row.upstream_id,"upstream",providers.data?.find((provider)=>provider.id===row.upstream_id)?.name)} · {protocolName(row.api_format)} · {new URL(row.base_url).host}{row.enabled?"":" · 已停用"}</option>)}</select></label>
    {endpoints.isError||providers.isError?<p role="alert">{asAppError(endpoints.error??providers.error).message}</p>:null}
    {endpoints.hasNextPage?<button type="button" className="secondary" disabled={endpoints.isFetching||submitted.current} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}
    {!rows.length&&!endpoints.isFetching?<Link to="/upstreams?add=provider" onClick={onClose}>先添加提供商</Link>:null}
    <label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置目录未收录的模型；已确认账号可使用</label>
    <p className="muted">保存后可在客户端密钥中选择此模型。</p>
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
  </form></Sheet>;
}
