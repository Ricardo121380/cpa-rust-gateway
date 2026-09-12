import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Sheet } from "../../components/Sheet";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import { connectModel } from "../models/connectModel";
import { CONNECTION_PRESETS, providerAddress } from "./connectionPresets";

export function ProviderDialog({onClose,onSaved}:Readonly<{onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
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
    const address=providerAddress(base.trim(),path.trim());
    const modelNames=[...new Set(models.split(/\r?\n/u).map((item)=>item.trim()).filter(Boolean))];
    if(modelNames.length>20||modelNames.some((item)=>item.length>256))throw new Error("一次最多添加 20 个模型，每个模型 ID 最长 256 字符。");
    if(modelNames.length&&!manual)throw new Error("请确认手动配置这些模型；目录中的模型也可以在模型目录页选择。");
    let material=secret.current?.value.trim()??"";
    if(secret.current)secret.current.value="";
    try {
      if(material.length>65536)throw new Error("凭据文件最多 64 KiB。");
      const task=await beginConfigurationTask(`添加提供商 · ${name.trim()}`);setWorkingId(task.version.id);
      const id=`provider-${crypto.randomUUID()}`,endpoint=`endpoint-${crypto.randomUUID()}`,policy=`policy-${crypto.randomUUID()}`;
      await task.mutate("createEgressPolicy",{body:{id:policy,name:`${name.trim()} · 连接`,allowed_schemes:["https"],allowed_hosts:[address.host],allowed_ports:[address.port],allowed_cidrs:[],redirect_mode:"deny",max_redirects:0}});
      await task.mutate("createUpstream",{body:{id,name:name.trim(),kind:preset.kind,enabled:true,tags:[],egress_policy_id:policy}});
      await task.mutate("createEndpoint",{path:{upstream_id:id},body:{id:endpoint,adapter_id:preset.adapter,api_format:preset.format,base_url:address.base,inference_path:path.trim(),models_path:null,transport:"https",enabled:true}});
      if(!preset.native&&material){
        const account=await task.mutate<{id:string}>("importChannelAccount",{path:{upstream_id:id},body:{id:`account-${crypto.randomUUID()}`,channel:preset.channel,secret:material}});
        material="";
        await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpoint},body:{credential_id:account.id,enabled:true,priority:0,weight:1,concurrency:1}});
      }
      for(const model of modelNames)await connectModel(task,{model,upstreamModel:model,endpointId:endpoint,allowUnlisted:manual});
      return task.finish();
    } finally {material="";}
  },onSuccess:onSaved});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  return <Sheet title="添加提供商" onEscape={()=>!save.isPending&&onClose()}><form className="sheet-form" onSubmit={submit}>
    <label>名称<input required maxLength={256} value={name} onChange={(event)=>setName(event.target.value)} placeholder="例如：我的 OpenAI" disabled={submitted.current}/></label>
    <label>渠道<select value={presetId} onChange={(event)=>changePreset(event.target.value)} disabled={submitted.current}>{CONNECTION_PRESETS.map((row)=><option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
    <label>接口地址<input type="url" required value={base} onChange={(event)=>setBase(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    <label>请求路径<input required value={path} onChange={(event)=>setPath(event.target.value)} readOnly={preset.fixed} disabled={submitted.current}/></label>
    {preset.native?<p className="muted">使用账号管理中已保存的 {preset.name} 授权。</p>:<label>API Key / 授权文件<span className="entity-meta">可稍后在账号管理中添加。授权文件请粘贴完整内容。</span><textarea ref={secret} rows={3} autoComplete="off" spellCheck={false} maxLength={65536} disabled={submitted.current}/></label>}
    <label>开放模型（可选）<textarea rows={3} value={models} onChange={(event)=>setModels(event.target.value)} placeholder="每行一个真实模型 ID" disabled={submitted.current}/></label>
    {models.trim()?<label className="check-row"><input type="checkbox" checked={manual} onChange={(event)=>setManual(event.target.checked)} disabled={submitted.current}/>手动配置这些模型，不依赖自动目录；已确认账号可使用</label>:null}
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
    <div className="sheet-actions"><button type="button" className="secondary" disabled={save.isPending} onClick={onClose}>取消</button><button type="submit" disabled={submitted.current}>{save.isPending?"正在保存…":"保存并应用"}</button></div>
  </form></Sheet>;
}
