import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { InlineWorkspace } from "../../components/InlineWorkspace";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { useSessionStore } from "../../session/sessionStore";
import { asAppError } from "../../api/errors";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel } from "../models/connectModel";
import { CONNECTION_PRESETS, providerAddress } from "./connectionPresets";

export function ProviderDialog({onClose,onSaved}:Readonly<{onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
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
  const [presetId,setPresetId]=useState<string>("responses");
  const preset=CONNECTION_PRESETS.find((row)=>row.id===presetId)!;
  const [name,setName]=useState("");
  const [base,setBase]=useState<string>(preset.base);
  const [path,setPath]=useState<string>(preset.path);
  const [models,setModels]=useState("");
  const [manual,setManual]=useState(false);
  const [workingId,setWorkingId]=useState<string>();
  const secret=useRef<HTMLTextAreaElement>(null);
  const submitted=useRef(false);
  useEffect(()=>()=>{if(secret.current)secret.current.value="";},[]);
  const changePreset=(id:string)=>{const next=CONNECTION_PRESETS.find((row)=>row.id===id)!;setPresetId(id);setBase(next.base);setPath(next.path);if(secret.current)secret.current.value="";};
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
      const task=await beginConfigurationTask(`添加提供商 · ${name.trim()}`,{id:owner.context.configVersionId,revision:owner.context.revision},"deferred");setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
      const id=`provider-${crypto.randomUUID()}`,endpoint=`endpoint-${crypto.randomUUID()}`,policy=`policy-${crypto.randomUUID()}`;
      await task.mutate("createEgressPolicy",{body:{id:policy,name:`${name.trim()} · 连接`,allowed_schemes:["https"],allowed_hosts:[address.host],allowed_ports:[address.port],allowed_cidrs:[],redirect_mode:"deny",max_redirects:0}});
      setConfirmed(1);
      await task.mutate("createUpstream",{body:{id,name:name.trim(),kind:preset.kind,enabled:true,tags:[],egress_policy_id:policy}});
      setConfirmed(2);
      await task.mutate("createEndpoint",{path:{upstream_id:id},body:{id:endpoint,adapter_id:preset.adapter,api_format:preset.format,base_url:address.base,inference_path:path.trim(),models_path:preset.native||preset.id==="kiro"||preset.id==="codex"?null:preset.format==="anthropic/messages"&&!address.base.endsWith("/v1")?"/v1/models":"/models",transport:"https",enabled:true}});
      setConfirmed(3);
      if(!preset.native&&material){
        const account=await task.mutate<{id:string}>("importChannelAccount",{path:{upstream_id:id},body:{id:`account-${crypto.randomUUID()}`,channel:preset.channel,secret:material}});
        material="";
        await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpoint},body:{credential_id:account.id,enabled:true,priority:0,weight:1,concurrency:1}});
      }
      for(const model of modelNames)await connectModel(task,{upstreamModel:model,endpointId:endpoint,allowUnlisted:manual});
      return task.finish();
    } finally {material="";}
  },onSuccess:version=>{if(owned()){useVersionStore.getState().rememberPending(version);if(useVersionStore.getState().context?.configVersionId===version.id)useVersionStore.getState().advanceFromEtag(version.revision);setReceipt(version);setDirty(false);}}});
  const submit=(event:FormEvent)=>{
    event.preventDefault();if(submitted.current)return;
    try {
      providerAddress(base.trim(),path.trim());
      if(!name.trim())throw new Error("请填写提供商名称。");
      const rows=[...new Set(models.split(/\r?\n/u).map(value=>value.trim()).filter(Boolean))];
      if(rows.length>20||rows.some(value=>value.length>256))throw new Error("一次最多添加 20 个模型，每个模型 ID 最长 256 字符。");
      if(rows.length&&!manual)throw new Error("请确认手动配置这些模型。");
      if((secret.current?.value.trim().length??0)>65536)throw new Error("凭据文件最多 64 KiB。");
    } catch(error){setValidation(asAppError(error).message);return;}
    setValidation(undefined);submitted.current=true;save.mutate();
  };
  return <InlineWorkspace title={receipt?"提供商创建结果":"添加 AI 提供商"} description="配置服务地址与协议；账号授权与凭据维护在账号管理中完成。" onClose={onClose} busy={save.isPending} dirty={dirty&&!submitted.current} footer={receipt?<><button className="secondary" onClick={()=>boundary.request(()=>{})}>关闭</button><button onClick={()=>review(receipt)}>查看工作草稿</button></>:<><button className="secondary" disabled={save.isPending} onClick={()=>boundary.request(()=>{})}>取消</button><button type="submit" form={formId} disabled={submitted.current}>{save.isPending?"正在保存…":"保存到草稿"}</button></>}>
    {receipt?<p role="status">提供商及接口已保存到草稿，尚未应用。请在待应用变更中统一核对并应用。</p>:<form id={formId} className="sheet-form" onSubmit={submit} onChange={()=>setDirty(true)}>
    {validation?<p role="alert">{validation}</p>:null}
    <fieldset className="workflow-section"><legend>服务</legend><label>名称<input required maxLength={256} value={name} onChange={(event)=>setName(event.target.value)} placeholder="例如：我的 OpenAI" disabled={submitted.current}/></label>
    <label>渠道<select value={presetId} onChange={(event)=>changePreset(event.target.value)} disabled={submitted.current}>{CONNECTION_PRESETS.map((row)=><option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
    </fieldset><fieldset className="workflow-section"><legend>连接</legend><label>接口地址<input type="url" required value={base} onChange={(event)=>setBase(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    <label>请求路径<input required value={path} onChange={(event)=>setPath(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    </fieldset><fieldset className="workflow-section"><legend>账号与模型（可选）</legend>{preset.native?<p className="muted">使用账号管理中已保存的 {preset.name} 授权。</p>:<label>API Key / 授权文件<span className="entity-meta">可稍后在账号管理中添加。授权文件请粘贴完整内容。</span><textarea ref={secret} rows={3} autoComplete="off" spellCheck={false} maxLength={65536} disabled={submitted.current}/></label>}
    <label>开放模型（可选）<textarea rows={3} value={models} onChange={(event)=>setModels(event.target.value)} placeholder="每行一个真实模型 ID" disabled={submitted.current}/></label>
    {models.trim()?<label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置这些模型，不依赖自动目录；已确认账号可使用</label>:null}
    </fieldset><ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={review}/>
    {save.isError?<p role="status">已确认保存 {confirmed} 项基础资源；账号与模型结果请读取工作草稿核对，本次操作不会重复提交。</p>:null}
  </form>} </InlineWorkspace>;
}
