import "../../components/workflow-forms.css";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { protocolName } from "../accounts/presentation";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel, parseModelIds } from "./connectModel";
import { runModelTask, type ModelTaskReceipt } from "./modelTask";

export function ConnectModelDialog({seed,endpointSeed,targetModelId,onClose,onSaved}:Readonly<{seed?:{model:string;endpoint:string;alias?:string};endpointSeed?:string;targetModelId?:string;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const formId="connect-model-form";
  const context=useVersionStore((state)=>state.context);
  const scope=context?.configVersionId;
  const endpoints=useManagedInventory("endpoints");
  const providers=useQuery({queryKey:["upstreams",scope,context?.revision],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const rows=endpoints.data?.pages.flatMap((page)=>page.items)??[];
  const [alias,setAlias]=useState(seed?.alias??"");
  const [customAlias,setCustomAlias]=useState(!!seed?.alias);
  const [upstreamModel,setUpstreamModel]=useState(seed?.model??"");
  const [endpoint,setEndpoint]=useState(seed?.endpoint??endpointSeed??"");
  const [manual,setManual]=useState(false);
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const submitted=useRef(false);
  const save=useMutation({mutationFn:async()=>{
    const names=parseModelIds(upstreamModel);
    if(targetModelId&&names.length!==1)throw new Error("为现有模型添加来源时，请只填写一个上游模型 ID。");
    if(customAlias && (names.length!==1 || !alias.trim()))throw new Error("自定义别名仅适用于单个模型。");
    if(!rows.some(row=>row.id===endpoint))throw new Error("所选接口尚未载入或已改变，请重新选择。");
    return runModelTask(`接入模型 · ${names.length} 个`,async(task)=>{
      for(const model of names)await connectModel(task,{upstreamModel:model,alias:customAlias?alias.trim():undefined,targetModelId,endpointId:endpoint,allowUnlisted:manual});
    },{probeUnchanged:true});
  },onSuccess:({receipt:next})=>setReceipt(next),onError:()=>{submitted.current=false;}});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  const done=()=>{if(receipt&&receipt.kind!=="unchanged")onSaved(receipt.workingVersion);else onClose();};
  return <Sheet title={receipt?"模型接入结果":"接入模型"} description={receipt?"请核对本次写入与应用状态，再继续管理模型。":"使用上游返回的完整模型 ID，并明确选择它应连接的接口。"} onEscape={()=>!save.isPending&&done()} busy={save.isPending} footer={receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="saved_applied"||receipt.kind==="unchanged"?"完成":"核对配置"}</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={submitted.current||!scope}>{save.isPending?"正在保存…":context?.status==="draft"?"保存到草稿":"保存并应用"}</button></>}>
    {receipt?<div role="status"><p>{receipt.message}</p><p>已确认保存 {receipt.acknowledgedWrites} 步。</p></div>:<form id={formId} className="sheet-form" onSubmit={submit}>
    <fieldset className="workflow-section"><legend>模型</legend><label>上游模型 ID<textarea required rows={4} maxLength={5140} value={upstreamModel} onChange={(event)=>setUpstreamModel(event.target.value)} disabled={submitted.current} placeholder="每行一个完整模型 ID，最多 20 个"/></label>
    <p className="stat-sub">客户端直接使用这些模型 ID。同名模型会添加来源连接。</p>
    <details className="model-advanced" open={seed?.alias?true:undefined}><summary>自定义模型别名</summary><label className="check-row"><input type="checkbox" checked={customAlias} onChange={(event)=>setCustomAlias(event.target.checked)} disabled={submitted.current}/>使用不同的客户端名称</label>{customAlias?<label>别名<input required maxLength={256} value={alias} onChange={(event)=>setAlias(event.target.value)} disabled={submitted.current}/></label>:null}</details>
    </fieldset><fieldset className="workflow-section"><legend>提供商连接</legend><label>接口连接<select required value={endpoint} onChange={(event)=>setEndpoint(event.target.value)} disabled={submitted.current}><option value="">选择提供商的接口</option>{rows.map((row)=><option key={row.id} value={row.id}>{resourceName(row.upstream_id,"upstream",providers.data?.find((provider)=>provider.id===row.upstream_id)?.name)} · {protocolName(row.api_format)} · {new URL(row.base_url).host}{row.enabled?"":" · 已停用"}</option>)}</select></label>
    {endpoints.isError||providers.isError?<p role="alert">{asAppError(endpoints.error??providers.error).message}</p>:null}
    {endpoints.hasNextPage?<button type="button" className="secondary" disabled={endpoints.isFetching||submitted.current} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}
    {!rows.length&&!endpoints.isFetching?<Link to="/upstreams?add=provider" onClick={onClose}>先添加提供商</Link>:null}
    <label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置目录未收录的模型；已确认账号可使用</label>
    </fieldset><p className="muted">保存后可在客户端密钥中选择此模型。</p>
    {save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
  </form>}
  </Sheet>;
}
