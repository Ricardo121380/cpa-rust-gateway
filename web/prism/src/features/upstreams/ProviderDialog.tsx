import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { InlineWorkspace } from "../../components/InlineWorkspace";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { useSessionStore } from "../../session/sessionStore";
import { asAppError } from "../../api/errors";
import { call } from "../../api/client";
import { matchingApiConnections, chooseApiConnection, readApiConnections } from "./apiProviderModel";
import { ConfigurationTaskReview } from "../config-versions/ConfigurationTaskReview";
import { beginConfigurationTask, type ConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel } from "../models/connectModel";
import { CONNECTION_PRESETS, providerAddress } from "./connectionPresets";

export function ProviderDialog({onClose,onSaved,channel,onConnectionCreated,existingUpstream}:Readonly<{onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void;channel?:string;existingUpstream?:{id:string;name:string};onConnectionCreated?:(version:ConfigVersionSummary,target:{upstream_id:string;endpoint_id:string})=>void}>) {
  const boundary=useOperationBoundary();
  const [owner]=useState(()=>({session:useSessionStore.getState().generation,selection:useVersionStore.getState().selectionGeneration,context:useVersionStore.getState().context}));
  const live=useRef(true);
  useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const owned=()=>live.current&&owner.session===useSessionStore.getState().generation&&owner.selection===useVersionStore.getState().selectionGeneration;
  const [dirty,setDirty]=useState(false);
  const [validation,setValidation]=useState<string>();
  const [receipt,setReceipt]=useState<ConfigVersionSummary>();
  const [confirmed,setConfirmed]=useState(0);
  const review=(version:ConfigVersionSummary)=>{if(owned()){useVersionStore.getState().rememberPending(version);onSaved(version);}};
  const formId="provider-setup-form";
  const availablePresets=CONNECTION_PRESETS.filter(row=>channel?row.channel===channel:["openai-compatible","anthropic-compatible","kimi-api","grok.official"].includes(row.channel));
  const [presetId,setPresetId]=useState<string>(availablePresets[0]!.id);
  const createdTarget=useRef<{upstream_id:string;endpoint_id:string}|undefined>(undefined);
  const preset=CONNECTION_PRESETS.find((row)=>row.id===presetId)!;
  const [name,setName]=useState(existingUpstream?.name??(channel?availablePresets[0]!.name:""));
  const [base,setBase]=useState<string>(preset.base);
  const [path,setPath]=useState<string>(preset.path);
  const [models,setModels]=useState("");
  const [manual,setManual]=useState(false);
  const [workingId,setWorkingId]=useState<string>();
  const [task,setTask]=useState<ConfigurationTask>();
  const [accountId,setAccountId]=useState<string>();
  const requestedAccount=useRef<{id:string;revision:string}|undefined>(undefined);
  const [connected,setConnected]=useState(false);
  const [selectedConnection,setSelectedConnection]=useState("");
  const [reviewBusy,setReviewBusy]=useState(false);
  const context=useVersionStore(state=>state.context);
  const pending=useVersionStore(state=>state.pending);
  const inventory=useQuery({queryKey:["api-provider-connections",pending?.id??context?.configVersionId,pending?.revision??context?.revision],enabled:!!context,retry:false,queryFn:()=>readApiConnections((operation,request)=>call(operation,{...request,headers:{...request?.headers,"X-Config-Version":pending?.id??context!.configVersionId}}))});
  const matches=inventory.data?matchingApiConnections(inventory.data.providers,inventory.data.endpoints,{kind:preset.kind,adapter:preset.adapter,format:preset.format,base,path}).filter(row=>!existingUpstream||row.provider.id===existingUpstream.id):[];

  const secret=useRef<HTMLTextAreaElement>(null);
  const submitted=useRef(false);
  useEffect(()=>()=>{if(secret.current)secret.current.value="";},[]);
  const changePreset=(id:string)=>{const next=CONNECTION_PRESETS.find((row)=>row.id===id)!;setPresetId(id);setSelectedConnection("");setBase(next.base);setPath(next.path);if(secret.current)secret.current.value="";};
  const save=useMutation({gcTime:0,mutationFn:async()=>{
    if(!owned()||!owner.context)throw new Error("配置上下文已失效，请重新打开。");
    const address=providerAddress(base.trim(),path.trim());
    const modelNames=[...new Set(models.split(/\r?\n/u).map((item)=>item.trim()).filter(Boolean))];
    if(modelNames.length>20||modelNames.some((item)=>item.length>256))throw new Error("一次最多添加 20 个模型，每个模型 ID 最长 256 字符。");
    if(modelNames.length&&!manual)throw new Error("请确认手动配置这些模型；目录中的模型也可以在模型目录页选择。");
    let material=secret.current?.value.trim()??"";
    if(secret.current)secret.current.value="";
    try {
      if(material.length>65536)throw new Error("凭据文件最多 64 KiB。");
      const task=await beginConfigurationTask(`添加提供商 · ${name.trim()}`,{id:owner.context.configVersionId,revision:owner.context.revision});setTask(task);setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
      const currentInventory=await readApiConnections(task.read);
      const candidates=matchingApiConnections(currentInventory.providers,currentInventory.endpoints,{kind:preset.kind,adapter:preset.adapter,format:preset.format,base:address.base,path:path.trim()}).filter(row=>!existingUpstream||row.provider.id===existingUpstream.id);
      const match=chooseApiConnection(candidates,selectedConnection||undefined);
      const id=match?.provider.id??existingUpstream?.id??`provider-${crypto.randomUUID()}`,endpoint=match?.endpoint.id??`endpoint-${crypto.randomUUID()}`,policy=`policy-${crypto.randomUUID()}`;
      if(!match&&!existingUpstream){
        await task.mutate("createEgressPolicy",{body:{id:policy,name:`${name.trim()} · 连接`,allowed_schemes:["https"],allowed_hosts:[address.host],allowed_ports:[address.port],allowed_cidrs:[],redirect_mode:"deny",max_redirects:0}});setConfirmed(1);
        await task.mutate("createUpstream",{body:{id,name:name.trim(),kind:preset.kind,enabled:true,tags:[],egress_policy_id:policy}});setConfirmed(2);
      }
      if(!match){await task.mutate("createEndpoint",{path:{upstream_id:id},body:{id:endpoint,adapter_id:preset.adapter,api_format:preset.format,base_url:address.base,inference_path:path.trim(),models_path:preset.native||preset.id==="kiro"||preset.id==="codex"?null:preset.format==="anthropic/messages"&&!address.base.endsWith("/v1")?"/v1/models":"/models",transport:"https",enabled:true}});setConfirmed(previous=>previous+1);}
      createdTarget.current={upstream_id:id,endpoint_id:endpoint};
      if(!preset.native&&material){
        requestedAccount.current={id:`account-${crypto.randomUUID()}`,revision:task.revision()};
        const account=await task.mutate<{id:string}>("importChannelAccount",{path:{upstream_id:id},body:{id:requestedAccount.current.id,channel:preset.channel,secret:material}});
        material="";setAccountId(account.id);
        const bindings=await task.read<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:endpoint}});
        if(!bindings.some(binding=>binding.credential_id===account.id))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpoint},body:{credential_id:account.id,enabled:true,priority:0,weight:1,concurrency:1}});
        setConnected(true);
      }
      for(const model of modelNames)await connectModel(task,{upstreamModel:model,endpointId:endpoint,allowUnlisted:manual});
      createdTarget.current={upstream_id:id,endpoint_id:endpoint};
      return task.finish();
    } finally {material="";}
  },onSuccess:version=>{if(owned()){useVersionStore.getState().rememberPending(version);if(useVersionStore.getState().context?.configVersionId===version.id)useVersionStore.getState().advanceFromEtag(version.revision);setReceipt(version);setDirty(false);}}});
  const readSaved=useMutation({mutationFn:async()=>{
    if(!owned())throw new Error("配置上下文已失效，请重新打开。");
    if(!task||!requestedAccount.current||!createdTarget.current)throw new Error("缺少本次账号目标，结果仍待确认。");
    const version=await task.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});
    try{
      let savedId=requestedAccount.current.id;
      try{
        const receipt=await task.read<{state:string;config_version:string;credential_id:string;upstream_id:string;started_revision:string}>("getAccountImportReceipt",{path:{upstream_id:createdTarget.current.upstream_id,import_id:requestedAccount.current.id},query:{started_revision:requestedAccount.current.revision}});
        if(receipt.state!=="completed"||receipt.config_version!==task.version.id||receipt.upstream_id!==createdTarget.current.upstream_id||receipt.started_revision!==requestedAccount.current.revision)throw new Error("导入回执与当前操作不一致，结果保持待确认。");
        savedId=receipt.credential_id;
      }catch(cause){if(asAppError(cause).status!==404)throw cause;}
      const account=await task.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:savedId}});
      if(!owned())throw new Error("配置上下文已失效，请重新打开。");
      if(account.id!==savedId||account.upstream_id!==createdTarget.current.upstream_id)throw new Error("账号所有权与本次目标不同。");
      setAccountId(account.id);setValidation("已确认工作草稿中存在账号，请核对完整清单后继续连接。");
    }
    catch(cause){if(!owned())throw cause;if(asAppError(cause).status===404&&version.revision===requestedAccount.current.revision)setValidation("已核对目标和版本：API 授权未保存。基础资源保留，可关闭后重新接入。");else throw cause;}
  },onError:cause=>{if(owned())setValidation(`仍待确认：${asAppError(cause).message}；不会重放 API Key 导入。`);}});
  const submit=(event:FormEvent)=>{
    event.preventDefault();if(submitted.current)return;
    try {
      providerAddress(base.trim(),path.trim());
      if(!inventory.data||inventory.isFetching)throw new Error("请先完整读取已有连接。");
      chooseApiConnection(matches,selectedConnection||undefined);
      if(!onConnectionCreated&&!secret.current?.value.trim())throw new Error("请输入 API Key。");
      if(!name.trim())throw new Error("请填写提供商名称。");
      const rows=[...new Set(models.split(/\r?\n/u).map(value=>value.trim()).filter(Boolean))];
      if(rows.length>20||rows.some(value=>value.length>256))throw new Error("一次最多添加 20 个模型，每个模型 ID 最长 256 字符。");
      if(rows.length&&!manual)throw new Error("请确认手动配置这些模型。");
      if((secret.current?.value.trim().length??0)>65536)throw new Error("凭据文件最多 64 KiB。");
    } catch(error){setValidation(asAppError(error).message);return;}
    setValidation(undefined);submitted.current=true;save.mutate();
  };
  return <InlineWorkspace title={receipt?"提供商创建结果":"添加 AI 提供商"} description="填写地址、API Key 与协议完成接入。模型权限可稍后独立设置。" onClose={()=>receipt&&onConnectionCreated&&createdTarget.current?onConnectionCreated(receipt,createdTarget.current):onClose()} busy={save.isPending||readSaved.isPending||reviewBusy} dirty={dirty&&!submitted.current} footer={receipt?<><button className="secondary" onClick={()=>boundary.request(()=>{})}>关闭</button><button onClick={()=>onConnectionCreated&&createdTarget.current?onConnectionCreated(receipt,createdTarget.current):review(receipt)}>{onConnectionCreated?"返回账号接入":"查看工作草稿"}</button></>:<><button className="secondary" disabled={save.isPending||readSaved.isPending} onClick={()=>boundary.request(()=>{})}>取消</button><button type="submit" className="primary" form={formId} disabled={submitted.current}>{save.isPending?"正在保存…":onConnectionCreated?"保存连接":"保存并接入"}</button></>}>
    {receipt?<p role="status">{receipt.status==="active"?"接入修改已应用；原有停用状态保持不变。请单独设置模型权限。":"服务与接口已保存到草稿，尚未应用。"}</p>:<form id={formId} className="sheet-form" onSubmit={submit} onChange={()=>setDirty(true)}>
    {validation?<p role="alert">{validation}</p>:null}
    <fieldset className="workflow-section"><legend>服务</legend><label>名称<input required maxLength={256} value={name} readOnly={!!existingUpstream} onChange={(event)=>setName(event.target.value)} placeholder="例如：我的 OpenAI" disabled={submitted.current}/></label>
    <label>渠道<select value={presetId} onChange={(event)=>changePreset(event.target.value)} disabled={submitted.current}>{availablePresets.map((row)=><option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
    </fieldset><fieldset className="workflow-section"><legend>连接</legend><label>接口地址<input type="url" required value={base} onChange={(event)=>setBase(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    <label>请求路径<input required value={path} onChange={(event)=>setPath(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    <p role="status">{inventory.isFetching?"正在读取已有连接…":inventory.isError?"连接读取失败，暂不能保存。":matches.length===0?"没有匹配连接，将新建。":matches.length===1?`将复用 ${matches[0]!.provider.name} · ${matches[0]!.endpoint.id}，保留原状态与连接设置。`:`找到 ${matches.length} 个匹配连接，请选择。`}</p>{matches.length>1?<label>复用连接<select required value={selectedConnection} onChange={event=>setSelectedConnection(event.target.value)} disabled={submitted.current}><option value="">请选择</option>{matches.map(row=><option key={row.endpoint.id} value={row.endpoint.id}>{row.provider.name} · {row.endpoint.id}{row.endpoint.enabled&&row.provider.enabled?"":" · 已停用"}</option>)}</select></label>:null}
    </fieldset><fieldset className="workflow-section"><legend>账号与模型</legend>{preset.native?<p className="muted">使用账号管理中已保存的 {preset.name} 授权。</p>:onConnectionCreated?<p className="muted">保存连接后返回账号接入，继续导入凭据。</p>:<label>API Key<span className="entity-meta">仅用于保存授权，不会触发推理请求。</span><textarea ref={secret} rows={3} autoComplete="off" spellCheck={false} maxLength={65536} disabled={submitted.current}/></label>}
    <label>开放模型（可选）<textarea rows={3} value={models} onChange={(event)=>setModels(event.target.value)} placeholder="每行一个真实模型 ID" disabled={submitted.current}/></label>
    {models.trim()?<label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置这些模型，不依赖自动目录；已确认账号可使用</label>:null}
    </fieldset><ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={review}/>
    {save.isError?<><p role="status">{accountId?`API 授权已保存（${accountId}）；连接或应用未完成。`:`已确认保存 ${confirmed} 项基础资源，API 授权尚未确认保存。`} 原保存回执保留，请先读取核对，本次操作不会重复提交。</p>{requestedAccount.current&&!accountId?<button type="button" className="secondary" disabled={readSaved.isPending} onClick={()=>readSaved.mutate()}>只读取 API 授权结果</button>:null}</>:null}
  </form>}
  {task&&(accountId||confirmed>0)&&receipt?.status!=="active"?<ConfigurationTaskReview task={task} onBusyChange={setReviewBusy} onApplied={version=>setReceipt(version)} onReviewed={accountId&&!connected?async review=>{
    await task.acceptReviewedDraft(review);const target=createdTarget.current!;
    const current=await task.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:accountId}});if(current.upstream_id!==target.upstream_id)throw new Error("账号目标已变化。");
    const bindings=await task.read<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:target.endpoint_id}});
    if(!bindings.some(binding=>binding.credential_id===accountId))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:target.endpoint_id},body:{credential_id:accountId,enabled:true,priority:0,weight:1,concurrency:1}});
    setConnected(true);return {...task.version,revision:task.revision()};
  }:undefined}/>:null}
  </InlineWorkspace>;
}
