import { useMutation, useQuery, isCancelledError, CancelledError } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { resourceName } from "../../utils/resourceNames";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { GrokDeviceWizard } from "./GrokDeviceWizard";
import { RuntimeApplyNotice } from "./RuntimeApplyNotice";
import { useManagedInventory } from "./inventory";
import { protocolName } from "./presentation";

type Channel=Readonly<{id:string;name:string;credential_format:string;import_available:boolean;authorization_available:boolean;upstream_kinds:readonly string[]}>;
type Material=Readonly<{id:string;label:string;secret:string}>;
type Result=Readonly<{id:string;label:string;status:string;error?:string}>;
type Imported=Readonly<{created:number;unchanged:number;runtime_applied?:boolean;identity_state?:string}>;
const formats:Record<string,string>={api_key:"API Key / Token",cpa_sub2api_json:"CPA / Sub2API / Codex 凭据 JSON",claude_json:"Claude 凭据 JSON 或 API Key",kiro_json_or_key:"Kiro 凭据 JSON 或 ksk_ Key",grok_build_json:"Grok Build 凭据 JSON",sso:"SSO 凭据"};
const MAX_FILES=20;

export function AddAccountDialog({onClose,onCreated}:Readonly<{onClose:()=>void;onCreated:(notice?:string)=>void}>) {
  const [channelId,setChannelId]=useState("openai-compatible");
  const [inputMode,setInputMode]=useState<"paste"|"files">("paste");
  const [oauth,setOauth]=useState(false);
  const [rows,setRows]=useState<Result[]>([]);
  const [completed,setCompleted]=useState(false);
  const [error,setError]=useState<string>();
  const [needsApply,setNeedsApply]=useState(false);
  const [finishedVersion,setFinishedVersion]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const [reading,setReading]=useState(false);
  const [providerId,setProviderId]=useState("");
  const [endpointId,setEndpointId]=useState<string|null>(null);
  const materials=useRef<Material[]>([]);
  const readerGeneration=useRef(0);
  const submitting=useRef(false);
  const secret=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>()=>{materials.current=[];readerGeneration.current+=1;},[]);
  const context=useVersionStore((state)=>state.context);
  const native=["grok.build","grok.console","grok.web"].includes(channelId);
  const channels=useQuery({queryKey:["account-channels"],queryFn:()=>call<readonly Channel[]>("listAccountChannels")});
  const providers=useQuery({queryKey:["account-providers",context?.configVersionId],enabled:!!context&&!native,
    queryFn:()=>call<readonly {id:string;name:string;kind:string}[]>("listUpstreams",{},{versionScoped:true})});
  const channel=channels.data?.find((row)=>row.id===channelId);
  const matches=providers.data?.filter((provider)=>channel?.upstream_kinds.includes(provider.kind))??[];
  const selectedProvider=matches.find((row)=>row.id===providerId)?.id??matches[0]?.id??"";
  const endpoints=useManagedInventory("endpoints",selectedProvider,"",!native&&!!selectedProvider);
  const connections=endpoints.data?.pages.flatMap((page)=>page.items)??[];
  const selectedEndpoint=endpointId??(connections.length===1?connections[0]!.id:"");
  const create=useMutation({gcTime:0,mutationFn:async({provider,endpoint,items}:{provider:string;endpoint:string;items:Material[]})=>{
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    const task=native?undefined:await beginConfigurationTask("导入账号");
    if(task)setWorkingId(task.version.id);
    const results:Result[]=items.map(({id,label})=>({id,label,status:"未执行"}));
    let saved=0;let canApply=true;
    for(const [index,item] of items.entries()) {
      assertOwner();
      let accountSaved=false;
      try {
        const body={id:item.id,channel:channelId,secret:item.secret};
        if(native) {
          const result=await call<Imported>("importNativeAccount",{body});
          results[index]={id:item.id,label:item.label,status:result.created?"已添加":"已存在",...(result.identity_state==="unavailable"?{error:"渠道暂未返回身份"}:{})};
          saved+=result.created;
          if(result.runtime_applied===false){setNeedsApply(true);canApply=false;setRows([...results]);break;}
        } else {
          const imported=await task!.mutate<{id:string}>("importChannelAccount",{path:{upstream_id:provider},body});
          accountSaved=true;saved+=1;
          if(endpoint){
            const bindings=await task!.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:endpoint}});
            if(!bindings.some((binding)=>binding.credential_id===imported.id))await task!.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpoint},body:{credential_id:imported.id,enabled:true,priority:0,weight:1,concurrency:1}});
          }
          results[index]={id:item.id,label:item.label,status:imported.id===item.id?"已添加":"已存在"};
        }
      } catch(cause) {
        if(isCancelledError(cause))throw cause;
        const failure=asAppError(cause);
        results[index]={id:item.id,label:item.label,status:accountSaved?"已保存，连接未完成":failure.kind==="network"?"结果未确认":"未添加",error:failure.message};
        // Independent invalid inputs may be skipped. A conflict/uncertain result never replays
        // a write or advances the remaining batch against a different source revision.
        if(accountSaved||failure.kind!=="invalid_request"){canApply=false;setRows([...results]);break;}
      }
      setRows([...results]);
    }
    if(task&&saved&&canApply) {
      try{setFinishedVersion(await task.finish());}
      catch(cause){if(isCancelledError(cause))throw cause;setError(asAppError(cause).message);}
    }
    return results;
  },onSuccess:(results)=>{setRows(results);setCompleted(true);},onError:(cause)=>{if(!isCancelledError(cause)){setError(asAppError(cause).message);setCompleted(true);}},onSettled:():void=>{submitting.current=false;create.reset();}});
  const review=useMutation({mutationFn:()=>call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}}),onSuccess:(version)=>{onClose();useVersionStore.getState().select(version);}});
  const busy=create.isPending||reading||review.isPending;
  const resetInput=()=>{materials.current=[];readerGeneration.current+=1;setReading(false);setRows([]);setError(undefined);if(secret.current)secret.current.value="";};
  const close=()=>{
    if(busy)return;
    materials.current=[];
    if(completed){
      const added=rows.filter((row)=>row.status==="已添加").length;
      const existing=rows.filter((row)=>row.status==="已存在").length;
      const remaining=rows.filter((row)=>!["已添加","已存在"].includes(row.status)).length;
      onCreated(needsApply?"账号已保存，运行配置暂未应用。":workingId&&!finishedVersion?"账号修改已保存，请核对应用状态。":finishedVersion?.status==="draft"?"账号已保存到待应用配置。":`已添加 ${added} 个账号${existing?`，${existing} 个已存在`:""}${remaining?`，${remaining} 个未完成`:""}。`);
      if(finishedVersion)useVersionStore.getState().select(finishedVersion);
    }
    else onClose();
  };
  const submit=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();if(submitting.current||busy)return;
    const form=new FormData(event.currentTarget);
    const items=inputMode==="files"?materials.current:[{id:`import-${crypto.randomUUID()}`,label:"粘贴的凭据",secret:secret.current?.value??""}];
    if(!items.length||items.some((item)=>!item.secret.trim()||new TextEncoder().encode(item.secret).length>65536)){setError("每份凭据须非空且不超过 64 KiB。");return;}
    materials.current=[];if(secret.current)secret.current.value="";setError(undefined);submitting.current=true;
    setRows(items.map(({id,label})=>({id,label,status:"待导入"})));
    create.mutate({provider:String(form.get("provider")??""),endpoint:String(form.get("endpoint")??""),items});
  };
  if(oauth)return <GrokDeviceWizard name="" onClose={()=>setOauth(false)} onComplete={onCreated}/>;
  return <Sheet title="添加账号" onEscape={close}>
    {completed?<>
      <h3>导入结果</h3>
      {needsApply?<RuntimeApplyNotice onApplied={()=>setNeedsApply(false)}/>:null}
    </>:channels.isPending?<p>读取接入方式…</p>:channels.isError?<p role="alert">{asAppError(channels.error).message}</p>:<>
      <label>渠道<select aria-label="渠道" value={channelId} disabled={busy} onChange={(event)=>{resetInput();setEndpointId(null);setProviderId("");setChannelId(event.target.value);}}>{channels.data?.map((entry)=><option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label>
      {channel?.authorization_available?<button type="button" disabled={busy} onClick={()=>{resetInput();setOauth(true);}}>授权登录</button>:null}
      {!native&&providers.isError?<p role="alert">{asAppError(providers.error).message}</p>:!native&&!!context&&providers.isPending?<p>读取提供商…</p>:!native&&!matches.length?<p><Link to={`/upstreams?add=provider&channel=${encodeURIComponent(channelId)}`} onClick={onClose}>添加 AI 提供商</Link>后，即可导入这类账号。</p>:channel?.import_available?<form className="sheet-form" onSubmit={submit} autoComplete="off">
        {!native?<><label>提供商<select name="provider" value={selectedProvider} onChange={(event)=>{setProviderId(event.target.value);setEndpointId(null);}} disabled={busy} required>{matches.map((provider)=><option key={provider.id} value={provider.id}>{resourceName(provider.id,"upstream",provider.name)}</option>)}</select></label>
          <label>接口连接<select name="endpoint" value={selectedEndpoint} onChange={(event)=>setEndpointId(event.target.value)} disabled={busy||endpoints.isFetching}><option value="">稍后连接</option>{connections.map((endpoint)=><option key={endpoint.id} value={endpoint.id}>{protocolName(endpoint.api_format)} · {new URL(endpoint.base_url).host}{endpoint.enabled?"":" · 已停用"}</option>)}</select></label>
          {endpoints.isError?<p role="alert">{asAppError(endpoints.error).message}。可以先保存账号，稍后连接接口。</p>:null}
          {endpoints.hasNextPage?<button type="button" className="secondary" disabled={busy||endpoints.isFetching} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}
        </>:null}
        <div className="page-actions"><button type="button" className="secondary" disabled={busy} aria-pressed={inputMode==="paste"} onClick={()=>{resetInput();setInputMode("paste");}}>粘贴凭据</button><button type="button" className="secondary" disabled={busy} aria-pressed={inputMode==="files"} onClick={()=>{resetInput();setInputMode("files");}}>选择文件</button></div>
        {inputMode==="paste"?<label>{formats[channel.credential_format]??"凭据"}<textarea ref={secret} name="secret" required maxLength={65536} disabled={busy} autoComplete="off" spellCheck={false} className="credential-input"/></label>:<label>凭据文件<input type="file" multiple disabled={busy} accept=".json,.txt,application/json,text/plain" onChange={async(event)=>{
          const files=Array.from(event.currentTarget.files??[]);event.currentTarget.value="";resetInput();
          if(!files.length)return;
          if(files.length>MAX_FILES||files.some((file)=>file.size>65536)||files.reduce((sum,file)=>sum+file.size,0)>1048576){setError("一次最多 20 份文件，每份 64 KiB，合计不超过 1 MiB。");return;}
          const owner=readerGeneration.current;setReading(true);
          try {
            const items=await Promise.all(files.map(async(file)=>({id:`import-${crypto.randomUUID()}`,label:file.name.slice(0,200),secret:await file.text()})));
            if(owner!==readerGeneration.current)return;
            materials.current=items;setRows(items.map(({id,label,secret})=>({id,label,status:secret.trim()?"待导入":"空文件"})));
          } catch {if(owner===readerGeneration.current)setError("无法读取文件，请重新选择。");}
          finally {if(owner===readerGeneration.current)setReading(false);}
        }}/></label>}
        <div className="sheet-actions"><button type="button" className="secondary" disabled={busy} onClick={close}>取消</button><button disabled={busy||(inputMode==="files"&&!materials.current.length)}>添加账号</button></div>
      </form>:<p>此渠道暂不可从面板接入。</p>}
    </>}
    {rows.length?<div className="tablewrap"><table><thead><tr><th>来源</th><th>结果</th></tr></thead><tbody>{rows.map((row)=><tr key={row.id}><td>{row.label}</td><td>{row.status}{row.error?<span className="entity-meta">{row.error}</span>:null}</td></tr>)}</tbody></table></div>:null}
    {error?<p role="alert">{error}</p>:null}
    {workingId&&completed&&!finishedVersion?<button className="secondary" disabled={busy} onClick={()=>review.mutate()}>查看待应用的修改</button>:null}
    {review.isError?<p role="alert">{asAppError(review.error).message}</p>:null}
    {completed?<div className="sheet-actions"><button disabled={busy} onClick={close}>完成</button></div>:null}
  </Sheet>;
}
