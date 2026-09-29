import {useEffect,useState} from "react";
import {useMutation,useQuery} from "@tanstack/react-query";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
import {KeyPermissionsDialog} from "../access/KeyPermissionsDialog";
import type {ClientKeyRecord} from "../access/model";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {useModelConnections} from "../models/useModelConnections";
import type {PublicModel} from "../models/model";
import type {AccountTarget} from "./AccountBatchDialog";
import {readNativeAccount,rememberCallReceipt,useAccountCallEvidence,type CallReceipt} from "./callEvidence";
import {useSessionStore} from "../../session/sessionStore";

/** Permission edits and explicit calls share identities, never side effects on opening. */
export function AccountModelsDialog({target,endpointIds,onClose,onSaved}:Readonly<{target:AccountTarget;endpointIds:readonly string[];onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>){
  const context=useVersionStore(state=>state.context);
  const topology=useModelConnections();
  const resources=useQuery({queryKey:["account-model-permissions",context?.configVersionId,context?.revision,target.id],enabled:!!context,retry:false,queryFn:async()=>{
    const models=await call<PublicModel[]>("listPublicModels",{},{versionScoped:true});
    const keys=await call<ClientKeyRecord[]>("listClientKeys",{},{versionScoped:true});
    return {models,keys};
  }});
  const [keyId,setKeyId]=useState("");const [editing,setEditing]=useState<ClientKeyRecord>();
  const [modelId,setModelId]=useState("");const [endpointId,setEndpointId]=useState("");
  const [protocol,setProtocol]=useState("openai_responses");const [mode,setMode]=useState("json");
  const [confirmed,setConfirmed]=useState(false);const [receipt,setReceipt]=useState<CallReceipt>();
  const recent=useAccountCallEvidence(target.id,target.native?target.nativeProvider:undefined,target.revision);
  useEffect(()=>{setConfirmed(false);setReceipt(undefined);setEditing(undefined);},[context?.configVersionId,context?.revision,target.id,keyId,modelId,endpointId,protocol,mode]);
  const routeIds=new Set(topology.data?.candidates.filter(candidate=>endpointIds.includes(candidate.endpoint_id)).map(candidate=>candidate.route_id)??[]);
  const routes=topology.data?.routes.filter(route=>routeIds.has(route.id))??[];
  const models=resources.data?.models.filter(model=>routes.some(route=>route.public_model_id===model.id))??[];
  const selectedModel=models.find(model=>model.id===modelId);
  const route=routes.find(route=>route.public_model_id===modelId);
  const endpoints=topology.data?.endpoints.filter(endpoint=>endpointIds.includes(endpoint.id)&&topology.data?.candidates.some(candidate=>candidate.route_id===route?.id&&candidate.endpoint_id===endpoint.id))??[];
  const key=resources.data?.keys.find(key=>key.id===keyId);
  const pin=useMutation({gcTime:0,mutationFn:async()=>{
    const session=useSessionStore.getState().generation;
    if(!context||context.status!=="active"||!key||!selectedModel||!route||!confirmed)throw new Error("请在当前活动配置中选择 Key、模型、协议与实际目标后确认。");
    const endpoint=endpoints.find(endpoint=>endpoint.id===endpointId);
    if(!endpoint||!endpoint.enabled||!target.enabled)throw new Error("所选账号或连接已停用，不能调用测试。");
    if(target.native){const current=await readNativeAccount(target.id,target.nativeProvider!);if(current.revision!==target.revision||!current.enabled||current.auth_status!=="active")throw new Error("原生账号授权或状态已变化，请重新打开。");}
    else{const current=await call<{id:string;upstream_id:string;revision:number}>("getCredential",{path:{credential_id:target.id}},{versionScoped:true});if(current.id!==target.id||current.upstream_id!==target.upstreamId||current.revision!==target.revision)throw new Error("账号授权已变化，请重新打开。");}
    if(useVersionStore.getState().context?.configVersionId!==context.configVersionId||useVersionStore.getState().context?.revision!==context.revision)throw new Error("配置已变化，请重新核对。");
    const value=await call<CallReceipt>("executeChannelPin",{headers:{"X-Config-Version":context.configVersionId,"If-Match":context.revision},body:{provider_id:endpoint.upstream_id,channel_id:endpoint.id,route_id:route.id,credential_id:target.id,credential_revision:target.revision,requested_model:selectedModel.model_name,protocol,mode,client_key_id:key.id}});
    if(session===useSessionStore.getState().generation)rememberCallReceipt(value);
    return value;
  },onSuccess:value=>setReceipt(value),onSettled:()=>setConfirmed(false)});
  const evidence=useQuery({queryKey:["account-call-evidence",receipt?.observed_at_ms,receipt?.credential_id,context?.revision],enabled:!!receipt,retry:false,refetchOnWindowFocus:true,queryFn:async()=>{
    const credential=target.native?await readNativeAccount(target.id,target.nativeProvider!):await call<{revision:number}>("getCredential",{path:{credential_id:target.id}},{versionScoped:true});
    const system=await call<{server_instance:string;build:{build_revision:string;rust_version:string;build_target:string}}>("getSystemInformation");
    return !!receipt&&receipt.server_instance===system.server_instance&&receipt.runtime_build===`${system.build.build_revision}:${system.build.rust_version}:${system.build.build_target}`&&receipt.config_version_id===context?.configVersionId&&`rev-${receipt.config_revision}`===context?.revision&&credential.revision===receipt.credential_revision;
  }});
  if(editing)return <KeyPermissionsDialog record={editing} modelScope={models.map(model=>model.id)} onClose={()=>setEditing(undefined)} onSaved={onSaved}/>;
  return <Sheet title={`${target.name} · 模型`} description="目录和连接不会自动授予调用权限；默认未选模型不能调用。测试仅在明确点击后发起。" onEscape={onClose} busy={pin.isPending} footer={<SheetDismissButton disabled={pin.isPending}>关闭</SheetDismissButton>}>
    {topology.isError||resources.isError?<p role="alert">{asAppError(topology.error??resources.error).message}</p>:null}
    {topology.isPending||resources.isPending?<p role="status">正在完整读取模型连接与 Key…</p>:null}
    <label>客户端 Key<select value={keyId} disabled={pin.isPending} onChange={event=>setKeyId(event.target.value)}><option value="">请选择要配置或测试的 Key</option>{resources.data?.keys.map(key=><option key={key.id} value={key.id}>{key.prefix} · {key.id} · {key.status}</option>)}</select></label>
    <p>{models.length} 个已连接模型；授权按模型路由生效，同路由其他账号可能承接请求。</p>
    <button type="button" className="secondary" disabled={!key||models.length===0||pin.isPending} onClick={()=>setEditing(key)}>设置模型权限</button>
    {models.length===0?<p>此账号尚无模型连接，可先在模型目录选择或手动补充；不会覆盖已有路由或开放权限。</p>:null}
    <h4>显式调用测试</h4>
    <p>以所选 Key 的当前权限图执行固定短请求，不校验 Key 密钥持有者身份。读取页面不会发起推理。</p>
    <p>请求内容固定为短文本连通性检查，最多尝试一次；不会自动切换账号或重试。</p>
    {target.native&&target.nativeProvider!=="grok_build"?<p>此渠道需要额外浏览器或会话初始化，当前单次测试无法证明严格单请求，测试明确不支持；不会调用上游。</p>:<>
      <label>模型<select value={modelId} disabled={pin.isPending} onChange={event=>{setModelId(event.target.value);setEndpointId("");}}><option value="">请选择模型</option>{models.map(model=><option key={model.id} value={model.id}>{model.model_name}</option>)}</select></label>
      <label>实际连接<select value={endpointId} disabled={pin.isPending} onChange={event=>setEndpointId(event.target.value)}><option value="">请选择实际目标</option>{endpoints.map(endpoint=><option key={endpoint.id} value={endpoint.id}>{endpoint.base_url}{endpoint.inference_path} · {endpoint.id}{endpoint.enabled?"":" · 已停用"}</option>)}</select></label>
      <label>客户端协议<select value={protocol} disabled={pin.isPending} onChange={event=>setProtocol(event.target.value)}><option value="openai_responses">Responses</option><option value="openai_chat_completions">Chat Completions</option><option value="anthropic_messages">Messages</option></select></label>
      <label>返回方式<select value={mode} disabled={pin.isPending} onChange={event=>setMode(event.target.value)}><option value="json">JSON</option><option value="sse">流式 SSE</option></select></label>
      <label className="check-row"><input type="checkbox" checked={confirmed} disabled={pin.isPending} onChange={event=>setConfirmed(event.target.checked)}/>确认按上述 Key、模型、协议和实际目标发起一次短请求，可能消耗上游额度</label>
      <button type="button" disabled={!confirmed||pin.isPending||!key||!route||!endpointId||context?.status!=="active"} onClick={()=>pin.mutate()}>执行一次调用测试</button>
    </>}
    {pin.isError?<p role="alert">{asAppError(pin.error).message}。结果不重放，请重新核对后决定下一次测试。</p>:null}
    {!receipt&&recent.receipt?<section aria-label="最近一次明确调用"><h4>最近一次明确调用</h4><p>{recent.receipt.outcome} · {recent.receipt.requested_model} · {recent.receipt.protocol} · {recent.receipt.mode} · {new Date(recent.receipt.observed_at_ms).toLocaleString()}</p><p>账号 {recent.receipt.credential_id} / revision {recent.receipt.credential_revision} · 连接 {recent.receipt.channel_id} · Key {recent.receipt.client_key_id}</p><p>配置 {recent.receipt.config_version_id} / rev-{recent.receipt.config_revision} · 构建 {recent.receipt.runtime_build??"未提供"}</p><p>{recent.current.data===true&&!recent.current.isFetching&&!recent.current.isError?"仅证明上述范围；不会证明当前表单尚未测试的模型或协议。":recent.current.data===false?"授权、配置或运行实例已变化，旧成功不能证明当前状态。":"当前有效性待核对；读取不会再次调用上游。"}</p><p>此会话保留最近回执；退出或刷新后未保留的历史保持未观测。</p></section>:null}
    {receipt?<section role="status"><h4>{evidence.data===true?"该身份下的调用观测":"历史观测，当前有效性待核对"}</h4><p>{receipt.outcome} · 发出上游请求 {receipt.upstream_sent?"是":"否"} · {receipt.attempt_count} 次尝试</p><p>账号 {receipt.credential_id} / revision {receipt.credential_revision} · 连接 {receipt.channel_id} · 配置 {receipt.config_version_id} / rev-{receipt.config_revision}</p><p>Key {receipt.client_key_id} · 模型 {receipt.requested_model} · 协议 {receipt.protocol} · 返回 {receipt.mode} · 固定短文本</p><p>构建 {receipt.runtime_build??"未提供，不能作为当前构建证明"} · {new Date(receipt.observed_at_ms).toLocaleString()}</p>{evidence.data===false?<p>授权或配置已变化，旧成功不代表当前可调用。</p>:null}{evidence.isError?<p>观测回执保留，当前状态读取失败。</p>:null}</section>:null}
  </Sheet>;
}
