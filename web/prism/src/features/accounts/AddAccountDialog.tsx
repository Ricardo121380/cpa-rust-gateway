import { useLocation, useNavigate } from "react-router-dom";
import { ProviderDialog } from "../upstreams/ProviderDialog";
import { KiroDeviceDialog } from "./KiroDeviceDialog";
import { KimiDeviceDialog } from "./KimiDeviceDialog";
import { useMutation, useQuery, useQueryClient, isCancelledError, CancelledError } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { resourceName } from "../../utils/resourceNames";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { beginConfigurationTask, type ConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskReview } from "../config-versions/ConfigurationTaskReview";
import { AuthorizationCodeDialog } from "./AuthorizationCodeDialog";
import { GrokDeviceWizard } from "./GrokDeviceWizard";
import { RuntimeApplyNotice } from "./RuntimeApplyNotice";
import { useManagedInventory } from "./inventory";
import { protocolName } from "./presentation";

type Channel=Readonly<{id:string;name:string;credential_format:string;import_available:boolean;authorization_flow:string;authorization_available:boolean;upstream_kinds:readonly string[]}>;
type Material=Readonly<{id:string;label:string;secret:string}>;
type Result=Readonly<{id:string;label:string;status:string;saved?:boolean;credentialId?:string;beforeRevision?:string;error?:string}>;
type Imported=Readonly<{created:number;unchanged:number;updated?:number;runtime_applied?:boolean;identity_state?:string}>;
const formats:Record<string,string>={api_key:"API Key / Token",cpa_sub2api_json:"CPA / Sub2API / Codex 凭据 JSON",claude_json:"Claude 凭据 JSON 或 API Key",kiro_json_or_key:"Kiro 凭据 JSON 或 ksk_ Key",kimi_oauth:"Kimi OAuth 凭据 JSON",grok_build_json:"Grok Build 凭据 JSON",sso:"SSO 凭据"};
const MAX_FILES=20;
const API_CHANNELS=new Set(["openai-compatible","anthropic-compatible","grok.official","kimi-api"]);
const CHANNEL_OWNED_AUTHORIZATION=new Set(["codex","claude","kimi-coding","kiro"]);
const CHANNEL_OWNED_IMPORT=new Set(["codex","claude","kimi-coding","kiro"]);

/** Keeps target ownership in the coordinator instead of a hidden or absent form control. */
export function importTargetForChannel(isNative:boolean,isApi:boolean,selectedProvider:string,formProvider:FormDataEntryValue|null):string {
  if(isNative)return "";
  return isApi?String(formProvider??""):selectedProvider;
}

/** Only an explicit API-setup choice can bind immediately during import. */
export function importEndpointForChannel(isApi:boolean,formEndpoint:FormDataEntryValue|null):string {
  return isApi?String(formEndpoint??""):"";
}

/** API services are selected by the operator; a singleton inventory is not a default choice. */
export function selectedAccountProvider(providerId:string,providers:readonly {id:string}[]):string {
  return providers.some((provider)=>provider.id===providerId)?providerId:"";
}

/** Reads only the non-secret regional selector from transient Kiro JSON import material. */
export function kiroImportRegion(items:readonly Material[]):string {
  const regions=new Set<string>();
  for(const item of items){
    try {const value=JSON.parse(item.secret) as {auth_region?:unknown};if(typeof value.auth_region==="string"&&value.auth_region)regions.add(value.auth_region);}
    catch { /* The server remains the authority for malformed or raw key material. */ }
  }
  if(regions.size>1)throw new Error("一次导入的 Kiro 授权包含多个地区，请按地区分别导入。");
  return [...regions][0]??"us-east-1";
}

/** Ordinary imports resolve their owner server-side in the same revisioned task. */
export function preparedImportOperation(channel:string):"prepareCodexAccountTarget"|"prepareClaudeAccountTarget"|"prepareKimiAccountTarget"|"prepareKiroAccountTarget"|undefined {
  if(channel==="codex")return "prepareCodexAccountTarget";
  if(channel==="claude")return "prepareClaudeAccountTarget";
  if(channel==="kimi-coding")return "prepareKimiAccountTarget";
  if(channel==="kiro")return "prepareKiroAccountTarget";
  return undefined;
}

/** Codex and Claude preparation have no request body; only Kiro owns a region input. */
export function preparedImportRequest(
  operation:ReturnType<typeof preparedImportOperation>,
  kiroRegion:string|undefined,
) {
  return operation==="prepareKiroAccountTarget"?{body:{region:kiroRegion??"us-east-1"}}:undefined;
}

type ImportTask = Awaited<ReturnType<typeof beginConfigurationTask>>;

/** Use only the endpoint returned by this task's channel-owned preparation. */
export async function prepareImportConnection(task:ImportTask,channel:string,provider:string,endpoint:string,region?:string) {
  const operation=preparedImportOperation(channel);
  if(!operation)return {upstream_id:provider,endpoint_id:endpoint};
  const target=await task.mutate<{upstream_id:string;endpoint_id:string}>(operation,operation==="prepareKimiAccountTarget"&&provider?{query:{upstream_id:provider}}:preparedImportRequest(operation,region));
  if(!target.upstream_id||!target.endpoint_id)throw new Error("渠道接入准备结果不完整，未开始导入。");
  return target;
}

/** An existing connection, including a disabled one, belongs to the operator. */
export async function connectImportedAccount(task:ImportTask,endpoint:string,credential:string) {
  if(!endpoint)return;
  const bindings=await task.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:endpoint}});
  if(!bindings.some(binding=>binding.credential_id===credential))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpoint},body:{credential_id:credential,enabled:true,priority:0,weight:1,concurrency:1}});
}

export function AddAccountDialog({onClose,onCreated,mode}:Readonly<{onClose:()=>void;onCreated:(notice?:string)=>void;mode?:"import"}>) {
  const location=useLocation(),navigate=useNavigate();
  const [resume]=useState(()=>{const value=location.state?.accountConnection;return value?.version===useVersionStore.getState().context?.configVersionId?value:undefined;});

  const formId="account-import-form";
  const client=useQueryClient();
  const [setupConnection,setSetupConnection]=useState(false);
  const [connectionOwner,setConnectionOwner]=useState<{id:string;name:string}>();
  const [channelId,setChannelId]=useState<string>(resume?.channel??"openai-compatible");
  const [choosingChannel,setChoosingChannel]=useState(!resume);
  const [inputMode,setInputMode]=useState<"paste"|"files">("paste");
  const [oauth,setOauth]=useState(false);
  const [method,setMethod]=useState<"authorize"|"import">(mode??"authorize");
  const [rows,setRows]=useState<Result[]>([]);
  const [completed,setCompleted]=useState(false);
  const [error,setError]=useState<string>();
  const [needsApply,setNeedsApply]=useState(false);
  const [finishedVersion,setFinishedVersion]=useState<ConfigVersionSummary>();
  const [workingId,setWorkingId]=useState<string>();
  const [configurationTask,setConfigurationTask]=useState<ConfigurationTask>();
  const [importTarget,setImportTarget]=useState<{upstream_id:string;endpoint_id:string}>();
  const [reviewApplying,setReviewApplying]=useState(false);
  const [reading,setReading]=useState(false);
  const [providerId,setProviderId]=useState<string>(resume?.upstream_id??"");
  const [endpointId,setEndpointId]=useState<string|null>(resume?.endpoint_id??null);
  useEffect(()=>{
    const value=location.state?.accountConnection;
    if(!value||value.version!==useVersionStore.getState().context?.configVersionId)return;
    setChannelId(value.channel);setProviderId(value.upstream_id);setEndpointId(value.endpoint_id);setChoosingChannel(false);setSetupConnection(false);
    navigate(location.pathname+location.search,{replace:true,state:null});
  },[location,navigate]);
  const materials=useRef<Material[]>([]);
  const readerGeneration=useRef(0);
  const submitting=useRef(false);
  const secret=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>()=>{materials.current=[];readerGeneration.current+=1;},[]);
  const context=useVersionStore((state)=>state.context);
  const native=["grok.build","grok.console","grok.web"].includes(channelId);
  const channels=useQuery({queryKey:["account-channels"],queryFn:()=>call<readonly Channel[]>("listAccountChannels")});
  // Channel-owned device flows prepare their own canonical target only after the operator starts.
  // They never need to enumerate arbitrary Providers just to render the chooser.
  const providers=useQuery({queryKey:["account-providers",context?.configVersionId],enabled:!choosingChannel&&!!context&&!native&&(!CHANNEL_OWNED_AUTHORIZATION.has(channelId)||channelId==="kimi-coding"),
    queryFn:()=>call<readonly {id:string;name:string;kind:string}[]>("listUpstreams",{},{versionScoped:true})});
  const channel=channels.data?.find((row)=>row.id===channelId);
  const authorizing=channel?.authorization_available===true && method==="authorize";
  const matches=providers.data?.filter((provider)=>channel?.upstream_kinds.includes(provider.kind))??[];
  // Named account channels are owned by their channel, not by every relay that
  // happens to understand the same wire protocol.  Never pick the first row:
  // that silently attached Kimi credentials to Codex/Krill installations.
  // Even an API-key setup must name its service explicitly.  A singleton list is
  // still an internal inventory result, not an operator's routing decision.
  const selectedProvider=selectedAccountProvider(providerId,matches);
  const isApiChannel=API_CHANNELS.has(channelId);
  const requiresConfiguredTarget=!native&&!isApiChannel;
  const endpoints=useManagedInventory("endpoints",selectedProvider,"",!native&&!!selectedProvider);
  const connections=endpoints.data?.pages.flatMap((page)=>page.items)??[];
  // An endpoint is an advanced routing decision. Built-in account journeys must never bind an
  // account merely because the browser happened to observe one compatible connection.
  const selectedEndpoint=endpointId??"";
  const importFormAvailable=channel?.import_available===true
    && !channels.isPending
    && !channels.isError
    && !(!native&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&providers.isError)
    && !(!native&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&!!context&&providers.isPending)
    && !(requiresConfiguredTarget&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&(matches.length===0||matches.length>1));
  const create=useMutation({gcTime:0,mutationFn:async({provider,endpoint,items,kiroRegion,retry}:{provider:string;endpoint:string;items:Material[];kiroRegion?:string;retry?:boolean})=>{
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const assertOwner=()=>{if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});};
    const task=native?undefined:retry&&configurationTask?configurationTask:await beginConfigurationTask("导入账号");
    if(retry&&task){const current=await task.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});if(current.status!=="draft"||current.revision!==task.revision())throw new Error("重试前工作配置已变化，请先核对；不会重放导入。");}
    if(task){setWorkingId(task.version.id);setConfigurationTask(task);}
    const target=task?(retry&&importTarget?importTarget:await prepareImportConnection(task,channelId,provider,endpoint,kiroRegion)):undefined;
    if(target)setImportTarget(target);
    const results:Result[]=retry?[...rows]:items.map(({id,label})=>({id,label,status:"未执行"}));
    let saved=0;let canApply=true;
    for(const [index,item] of items.entries()) {
      assertOwner();
      const resultIndex=retry?results.findIndex(row=>row.id===item.id):index;
      if(resultIndex<0||(retry&&! ["未添加","未执行"].includes(results[resultIndex]?.status??"")))throw new Error("此项未确定未保存，不能重试。");
      let accountSaved=false;
      let credentialId:string|undefined;
      const beforeRevision=task?.revision();
      try {
        const body={id:item.id,channel:channelId,secret:item.secret};
        if(native) {
          const result=await call<Imported>("importNativeAccount",{body});
          results[resultIndex]={id:item.id,label:item.label,status:result.created?"已添加":result.updated?"已更新授权":"已存在",saved:true,...(result.identity_state==="unavailable"?{error:"渠道暂未返回身份"}:{})};
          saved+=result.created+result.unchanged+(result.updated??0);
          if(result.runtime_applied===false){setNeedsApply(true);canApply=false;setRows([...results]);break;}
        } else {
          const imported=await task!.mutate<{id:string}>("importChannelAccount",{path:{upstream_id:target!.upstream_id},body});
          accountSaved=true;credentialId=imported.id;saved+=1;
          await connectImportedAccount(task!,target!.endpoint_id,imported.id);
          results[resultIndex]={id:item.id,label:item.label,status:imported.id===item.id?"已添加":"已存在",saved:true,credentialId};
        }
      } catch(cause) {
        if(isCancelledError(cause))throw cause;
        const failure=asAppError(cause);
        const rejected=["invalid_request","conflict","session_invalid"].includes(failure.kind);
        results[resultIndex]={id:item.id,label:item.label,status:accountSaved?"已保存，连接未完成":rejected?"未添加":"结果未确认",saved:accountSaved,credentialId,beforeRevision,error:failure.message};
        // Independent invalid inputs may be skipped. A conflict/uncertain result never replays
        // a write or advances the remaining batch against a different source revision.
        canApply=false;
        if(accountSaved||failure.kind!=="invalid_request"){canApply=false;setRows([...results]);break;}
      }
      setRows([...results]);
    }
    if(task&&saved&&canApply&&results.every(row=>row.saved)) {
      try{setFinishedVersion(await task.finish());}
      catch(cause){if(isCancelledError(cause))throw cause;setError(asAppError(cause).message);}
    }
    return results;
  },onSuccess:(results)=>{setRows(results);setCompleted(true);},onError:(cause)=>{if(!isCancelledError(cause)){setError(asAppError(cause).message);setCompleted(true);}},onSettled:():void=>{submitting.current=false;create.reset();}});
  const review=useMutation({mutationFn:async()=>{
    if(native){
      const reconciled:Result[]=[];
      for(const row of rows){
        if(row.status!=="结果未确认"){reconciled.push(row);continue;}
        try {
          const receipt=await call<{batch_id:string;channel:string;account_id:string;saved:boolean;account_exists:boolean}>("getNativeAccountImportReceipt",{path:{batch_id:row.id}});
          if(receipt.batch_id!==row.id||receipt.channel!==channelId||!receipt.saved){reconciled.push(row);continue;}
          reconciled.push({...row,saved:true,credentialId:receipt.account_id,status:receipt.account_exists?"已回读账号，运行应用待核对":"已保存，账号随后移除",error:undefined});
          if(receipt.account_exists)setNeedsApply(true);
        } catch(cause){if(isCancelledError(cause))throw cause;reconciled.push(row);}
      }
      return {version:undefined,reconciled};
    }
    const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}});
    const reconciled:Result[]=[];
    for(const row of rows){
      if(row.saved||row.status!=="结果未确认"||!configurationTask||!importTarget){reconciled.push(row);continue;}
      try {
        let savedId=row.id;
        if(row.beforeRevision){
          try {const receipt=await configurationTask.read<{state:string;credential_id:string;upstream_id:string;started_revision:string}>("getAccountImportReceipt",{path:{upstream_id:importTarget.upstream_id,import_id:row.id},query:{started_revision:row.beforeRevision}});
            if(receipt.state==="completed"&&receipt.upstream_id===importTarget.upstream_id&&receipt.started_revision===row.beforeRevision)savedId=receipt.credential_id;
          }catch(cause){if(isCancelledError(cause))throw cause;}
        }
        const account=await configurationTask.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:savedId}});
        if(account.id!==savedId||account.upstream_id!==importTarget.upstream_id){reconciled.push(row);continue;}
        reconciled.push({...row,saved:true,credentialId:account.id,status:"已保存，连接与应用待核对",error:undefined});
      } catch(cause){
        if(isCancelledError(cause))throw cause;
        // A missing requested ID is not proof against server-side deduplication.
        // Only an unchanged configuration revision also proves no import committed.
        const failure=asAppError(cause);
        reconciled.push(failure.status===404&&version.revision===row.beforeRevision?{...row,status:"未添加",error:"已核对资源和版本，未保存"}:row);
      }
    }
    return {version,reconciled};
  },onSuccess:({version,reconciled})=>{
    setRows(reconciled);if(version)useVersionStore.getState().rememberPending(version);
    if(version?.status==="active"&&version.revision===configurationTask?.revision())setFinishedVersion(version);
    setError(reconciled.some(row=>row.status==="结果未确认")?"仍有结果待确认；不会重放导入，请保留资源和版本信息继续核对。":undefined);
  }});
  const continueConnection=useMutation({mutationFn:async(row:Result)=>{
    if(!row.saved||!row.credentialId||!configurationTask||!importTarget)throw new Error("缺少已确认的账号保存回执。");
    const version=await configurationTask.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:configurationTask.version.id}});
    if(version.status!=="draft"||version.revision!==configurationTask.revision())throw new Error("工作配置已有后续变化，请先核对完整清单；不会重新导入账号。");
    const account=await configurationTask.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:row.credentialId}});
    if(account.id!==row.credentialId||account.upstream_id!==importTarget.upstream_id)throw new Error("保存的账号所有权已变化，请重新核对。");
    await connectImportedAccount(configurationTask,importTarget.endpoint_id,row.credentialId);
    const next=rows.map(item=>item.id===row.id?{...item,status:"已保存，连接完成",error:undefined}:item);
    setRows(next);
    if(next.every(item=>item.saved&&!item.status.includes("未完成")&&!item.status.includes("待核对")))setFinishedVersion(await configurationTask.finish());
  },onError:cause=>{if(!isCancelledError(cause))setError(asAppError(cause).message);}});
  const retryFile=async(row:Result,file:File)=>{
    if(submitting.current||busy||!completed||! ["未添加","未执行"].includes(row.status))return;
    if(file.size>65536){setError("凭据文件最多 64 KiB。");return;}
    const owner=readerGeneration.current;setReading(true);
    try {const material=await file.text();if(owner!==readerGeneration.current)return;
      if(!material.trim())throw new Error("文件为空，尚未重试。");
      submitting.current=true;setError(undefined);setFinishedVersion(undefined);
      create.mutate({provider:importTarget?.upstream_id??selectedProvider,endpoint:importTarget?.endpoint_id??selectedEndpoint,items:[{id:row.id,label:row.label,secret:material}],kiroRegion:channelId==="kiro"?kiroImportRegion([{id:row.id,label:row.label,secret:material}]):undefined,retry:true});
    }catch(cause){setError(asAppError(cause).message);submitting.current=false;}finally{if(owner===readerGeneration.current)setReading(false);}
  };
  const busy=create.isPending||reading||review.isPending||continueConnection.isPending||reviewApplying;
  const resetInput=()=>{materials.current=[];readerGeneration.current+=1;setReading(false);setRows([]);setError(undefined);if(secret.current)secret.current.value="";};
  const close=()=>{
    if(busy)return;
    materials.current=[];
    if(completed){
      const added=rows.filter((row)=>row.status==="已添加").length;
      const existing=rows.filter((row)=>row.status==="已存在").length;
      const remaining=rows.filter((row)=>!["已添加","已存在"].includes(row.status)).length;
      const saved=rows.filter((row)=>row.saved===true).length;
      const uncertain=rows.some((row)=>row.status==="结果未确认");
      if(finishedVersion)useVersionStore.getState().select(finishedVersion);
      onCreated(!saved?(uncertain?"导入结果待确认，请先核对，不要重复导入。":"本次未保存账号，请查看失败结果。")
        :needsApply?"账号已保存，运行配置暂未应用。":workingId&&!finishedVersion?`已确认保存 ${saved} 个账号，请核对连接与应用状态。`:finishedVersion?.status==="draft"?"账号已保存到待应用配置。":`已添加 ${added} 个账号${existing?`，${existing} 个已存在`:""}${remaining?`，${remaining} 个未完成`:""}。`);
    }
    else onClose();
  };
  const submit=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();if(submitting.current||busy)return;
    const form=new FormData(event.currentTarget);
    const items=inputMode==="files"?materials.current:[{id:`import-${crypto.randomUUID()}`,label:"粘贴的凭据",secret:secret.current?.value??""}];
    if(!items.length||items.some((item)=>!item.secret.trim()||new TextEncoder().encode(item.secret).length>65536)){setError("每份凭据须非空且不超过 64 KiB。");return;}
    const provider=importTargetForChannel(native,isApiChannel,selectedProvider,form.get("provider"));
    const endpoint=importEndpointForChannel(isApiChannel,form.get("endpoint"));
    if(!native&&!provider&&!CHANNEL_OWNED_IMPORT.has(channelId)){setError("该渠道没有唯一的专用接入，未开始导入。");return;}
    let kiroRegion:string|undefined;
    try {if(channelId==="kiro")kiroRegion=kiroImportRegion(items);} catch(cause) {setError(asAppError(cause).message);return;}
    materials.current=[];if(secret.current)secret.current.value="";setError(undefined);submitting.current=true;
    setRows(items.map(({id,label})=>({id,label,status:"待导入"})));
    create.mutate({provider,endpoint,items,kiroRegion});
  };
  if(setupConnection)return <ProviderDialog channel={channelId} existingUpstream={connectionOwner} onClose={()=>setSetupConnection(false)} onSaved={version=>{useVersionStore.getState().select(version);setSetupConnection(false);}} onConnectionCreated={(version,target)=>{void client.invalidateQueries({queryKey:["account-providers"]});void client.invalidateQueries({queryKey:["managed-inventory"]});navigate(native?"/upstreams":location.pathname+location.search,{replace:true,state:native?null:{accountConnection:{...target,channel:channelId,version:version.id}}});useVersionStore.getState().select(version);}}/>;
  if(oauth&&channelId==="kiro")return <KiroDeviceDialog onClose={()=>setOauth(false)} onComplete={onCreated}/>;
  if(oauth&&channelId==="kimi-coding")return <KimiDeviceDialog providerId={selectedProvider||undefined} onClose={()=>setOauth(false)} onComplete={onCreated}/>;
  if(oauth&&(channelId==="codex"||channelId==="claude"))return <AuthorizationCodeDialog channel={channelId} onClose={()=>setOauth(false)} onComplete={onCreated}/>;
  if(oauth&&channelId==="grok.build")return <GrokDeviceWizard name="" onClose={()=>setOauth(false)} onComplete={onCreated}/>;
  if(oauth)return <Sheet title="暂不支持的授权方式" layout="confirm" description="该渠道没有可用的授权流程，面板没有执行任何操作。" onEscape={()=>setOauth(false)}><p role="alert">请返回并选择支持的渠道接入方式。</p><div className="sheet-actions"><SheetDismissButton>返回</SheetDismissButton></div></Sheet>;
  return <Sheet title={completed?"导入结果":choosingChannel?"添加账号":channel?.name??"添加账号"} description={choosingChannel?"选择要接入的渠道。":undefined} onEscape={close} busy={busy} footer={choosingChannel?<SheetDismissButton className="secondary">取消</SheetDismissButton>:completed?<SheetDismissButton disabled={busy}>完成</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button className="primary" type={authorizing?"button":"submit"} form={authorizing?undefined:formId} disabled={busy||(channelId==="kimi-coding"&&(providers.isPending||providers.isError||(matches.length>1&&!selectedProvider)))||(authorizing?(!native&&!selectedProvider&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)):(!importFormAvailable||(isApiChannel&&!selectedProvider)||(inputMode==="files"&&!materials.current.length)))} onClick={authorizing?()=>{resetInput();setOauth(true);}:undefined}>{authorizing?"授权登录":"导入账号"}</button></>}>
    {completed?<>
      {needsApply?<RuntimeApplyNotice onApplied={()=>setNeedsApply(false)}/>:null}
      {native?<><p role="status">授权保存与接口接入是独立步骤。请核对对应渠道的接口及开放模型后再使用。</p><button className="secondary" onClick={()=>setSetupConnection(true)}>配置渠道接口</button><a href="#/upstreams" onClick={close}>查看已有服务与连接</a></>:null}
    </>:channels.isPending?<p>读取接入方式…</p>:channels.isError?<p role="alert">{asAppError(channels.error).message}</p>:choosingChannel?<div className="account-channel-grid" aria-label="选择账号渠道">{channels.data?.filter(entry=>!mode||(!API_CHANNELS.has(entry.id)&&entry.import_available)).map(entry=><button type="button" className="secondary account-channel-option" key={entry.id} onClick={()=>{resetInput();setEndpointId(null);setProviderId("");setChannelId(entry.id);setMethod(mode??"authorize");setChoosingChannel(false);}}><span className="channel-symbol" aria-hidden="true">{entry.name.slice(0,1)}</span><span><strong>{entry.name}</strong><small>{entry.authorization_available?"官方授权":entry.import_available?"导入凭据":"暂不可接入"}</small></span></button>)}</div>:<>
      <SheetDismissButton className="secondary channel-back" disabled={busy} onDismiss={()=>{resetInput();setChoosingChannel(true);}}>更换渠道</SheetDismissButton>
      {!mode&&channel?.authorization_available&&channel.import_available?<div className="account-onboarding"><div className="account-access-method" role="group" aria-label="接入方式">
        <SheetDismissButton className="secondary" aria-pressed={authorizing} disabled={busy} onClick={event=>{if(authorizing)event.preventDefault();}} onDismiss={()=>{resetInput();setMethod("authorize");}}>官方授权</SheetDismissButton>
        <SheetDismissButton className="secondary" aria-pressed={!authorizing} disabled={busy} onClick={event=>{if(!authorizing)event.preventDefault();}} onDismiss={()=>{resetInput();setMethod("import");}}>导入凭据</SheetDismissButton>
      </div></div>:null}
      {channelId==="kimi-coding"&&matches.length>1?<label>接入服务<select value={selectedProvider} onChange={event=>setProviderId(event.target.value)} disabled={busy}><option value="">选择 Kimi Coding 服务</option>{matches.map(provider=><option key={provider.id} value={provider.id}>{resourceName(provider.id,"upstream",provider.name)}</option>)}</select></label>:null}
      {channelId==="kimi-coding"&&providers.isError?<p role="alert">{asAppError(providers.error).message}</p>:null}
      {authorizing?<div className="account-authorization-summary"><h3>登录 {channel?.name}</h3><p>按下一步提示完成官方登录，再回到面板查看授权结果。</p></div>:<>
      {!native&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&providers.isError?<p role="alert">{asAppError(providers.error).message}</p>:!native&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&!!context&&providers.isPending?<p>读取渠道配置…</p>:requiresConfiguredTarget&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&!matches.length?<p role="alert">此渠道尚未配置专用接入。账号授权不会借用其他渠道或兼容中转。</p>:requiresConfiguredTarget&&!CHANNEL_OWNED_AUTHORIZATION.has(channelId)&&matches.length>1?<p role="alert">此渠道有多个专用接入，请在 AI 提供商中整理渠道配置后再授权。为避免错误绑定，面板不会自动选择其中一个。</p>:channel?.import_available?<form id={formId} className="sheet-form" onSubmit={submit} autoComplete="off">
        {!native&&isApiChannel?<><button type="button" className="secondary" disabled={busy} onClick={()=>{resetInput();setConnectionOwner(undefined);setSetupConnection(true);}}>创建服务与接口</button>{!matches.length?<p role="status">尚无此渠道的服务，请先创建服务与接口，再导入凭据。</p>:null}<label>服务<select name="provider" value={selectedProvider} onChange={(event)=>{setProviderId(event.target.value);setEndpointId(null);}} disabled={busy} required><option value="">选择已配置服务</option>{matches.map((provider)=><option key={provider.id} value={provider.id}>{resourceName(provider.id,"upstream",provider.name)}</option>)}</select></label>
          <label>接口连接<select name="endpoint" value={selectedEndpoint} onChange={(event)=>setEndpointId(event.target.value)} disabled={busy||endpoints.isFetching}><option value="">稍后连接</option>{connections.map((endpoint)=><option key={endpoint.id} value={endpoint.id}>{protocolName(endpoint.api_format)} · {new URL(endpoint.base_url).host}{endpoint.enabled?"":" · 已停用"}</option>)}</select></label>
          {selectedProvider?<button type="button" className="secondary" disabled={busy} onClick={()=>{resetInput();setConnectionOwner(matches.find(provider=>provider.id===selectedProvider));setSetupConnection(true);}}>为此服务添加接口</button>:null}
          {endpoints.isError?<p role="alert">{asAppError(endpoints.error).message}。可以先保存账号，稍后连接接口。</p>:null}
          {endpoints.hasNextPage?<button type="button" className="secondary" disabled={busy||endpoints.isFetching} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}
        </>:null}
        <div className="account-access-method" role="group" aria-label="凭据来源"><SheetDismissButton className="secondary" disabled={busy} aria-pressed={inputMode==="paste"} onClick={event=>{if(inputMode==="paste")event.preventDefault();}} onDismiss={()=>{resetInput();setInputMode("paste");}}>粘贴凭据</SheetDismissButton><SheetDismissButton className="secondary" disabled={busy} aria-pressed={inputMode==="files"} onClick={event=>{if(inputMode==="files")event.preventDefault();}} onDismiss={()=>{resetInput();setInputMode("files");}}>选择文件</SheetDismissButton></div>
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
      </form>:<p>此渠道暂不可从面板接入。</p>}
      </>}
    </>}
    {rows.length?<div className="tablewrap"><table><thead><tr><th>来源</th><th>结果</th></tr></thead><tbody>{rows.map((row)=><tr key={row.id}><td>{row.label}</td><td>{row.status}{row.error?<span className="entity-meta">{row.error}</span>:null}{completed&&row.saved&&row.credentialId&&row.status==="已保存，连接未完成"?<button type="button" className="secondary" disabled={busy} onClick={()=>{setError(undefined);continueConnection.mutate(row);}}>继续连接此账号</button>:null}{completed&&!row.saved&&["未添加","未执行"].includes(row.status)?<label className="small">仅重试此项：重新选择材料<input type="file" aria-label={`重试 ${row.label}`} disabled={busy||needsApply} onChange={event=>{const file=event.currentTarget.files?.[0];event.currentTarget.value="";if(file)void retryFile(row,file);}}/></label>:null}</td></tr>)}</tbody></table></div>:null}
    {completed&&rows.some(row=>row.saved)?<p role="status">已确认保存 {rows.filter(row=>row.saved).length} 项授权。配置{finishedVersion?.status==="active"?"已生效":finishedVersion?.status==="draft"?"待应用":error?"校验／应用未完成，保存回执保留":"待核对"}；这不代表真实调用已验证。</p>:null}
    {completed&&configurationTask&&finishedVersion?.status!=="active"&&rows.some(row=>row.saved)&&!rows.some(row=>row.status==="结果未确认")?<ConfigurationTaskReview task={configurationTask} onBusyChange={setReviewApplying} onApplied={version=>{setFinishedVersion(version);setError(undefined);}} onReviewed={rows.some(row=>row.saved&&row.credentialId&&(row.status.includes("未完成")||row.status.includes("待核对")))?async review=>{
      await configurationTask.acceptReviewedDraft(review);
      for(const row of rows.filter(row=>row.saved&&row.credentialId&&(row.status.includes("未完成")||row.status.includes("待核对")))){
        const account=await configurationTask.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:row.credentialId!}});if(account.upstream_id!==importTarget?.upstream_id)throw new Error("账号所有权已变化。");
        await connectImportedAccount(configurationTask,importTarget!.endpoint_id,row.credentialId!);
        setRows(current=>current.map(item=>item.id===row.id?{...item,status:"已保存，连接完成",error:undefined}:item));
      }
      return {...configurationTask.version,revision:configurationTask.revision()};
    }:undefined}/>:null}
    {error?<p role="alert">{error}</p>:null}
    {(workingId||native)&&completed&&!finishedVersion?<button className="secondary" disabled={busy} onClick={()=>review.mutate()}>核对导入结果</button>:null}
    {review.isError?<p role="alert">{asAppError(review.error).message}</p>:null}
  </Sheet>;
}
