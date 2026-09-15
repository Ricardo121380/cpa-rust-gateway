import {useMutation} from "@tanstack/react-query";
import {useRef,useState} from "react";
import {Sheet} from "../../components/Sheet";
import {asAppError} from "../../api/errors";
import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {parseOAuthCallback,safeExternalUrl} from "../upstreams/model";

type Session={credential_id:string;state:string;authorization_url?:string|null;expires_at_ms?:number|null};
type Target=Readonly<{upstream_id:string;endpoint_id:string}>;
type Enrollment={task:Awaited<ReturnType<typeof beginConfigurationTask>>;id:string;providerId:string;endpointId:string};

/** Reauthorization always retains the exact owner; only first authorization prepares a target. */
export function existingAuthorizationTarget(providerId:string|undefined,endpointId:string|undefined):Target|undefined {
  return providerId?{upstream_id:providerId,endpoint_id:endpointId??""}:undefined;
}

export function AuthorizationCodeDialog({providerId,providerName,endpointId,onClose,onComplete,channel="codex",credentialId}:Readonly<{
  providerId?:string;providerName?:string;endpointId?:string;onClose:()=>void;onComplete:(notice?:string)=>void;channel?:"codex"|"claude";credentialId?:string;
}>) {
  const label=channel==="claude"?"Claude":"Codex";
  const providerLabel=channel==="claude"?"Claude":"OpenAI";
  const operations=channel==="claude"?{start:"startClaudeEnrollment",complete:"completeClaudeEnrollment",cancel:"cancelClaudeEnrollment"} as const:{start:"startCodexEnrollment",complete:"completeCodexEnrollment",cancel:"cancelCodexEnrollment"} as const;
  const current=useRef<Enrollment|undefined>(undefined);
  const [session,setSession]=useState<Session>();
  const [callback,setCallback]=useState("");
  const [inputError,setInputError]=useState<string>();
  const [workingId,setWorkingId]=useState<string>();
  const [completed,setCompleted]=useState<ConfigVersionSummary>();
  const start=useMutation({mutationFn:async()=>{
    const task=await beginConfigurationTask(`授权 ${label} 账号`);
    const id=credentialId??`${channel}-${crypto.randomUUID()}`;
    const target=existingAuthorizationTarget(providerId,endpointId)??await task.mutate<Target>(channel==="codex"?"prepareCodexAccountTarget":"prepareClaudeAccountTarget");
    current.current={task,id,providerId:target.upstream_id,endpointId:target.endpoint_id};setWorkingId(task.version.id);
    return task.read<Session>(operations.start,{path:{upstream_id:target.upstream_id},body:{id,replace_existing:!!credentialId},headers:{"If-Match":task.revision()}});
  },onSuccess:setSession});
  const complete=useMutation({gcTime:0,mutationFn:async()=>{
    const enrollment=current.current;if(!enrollment)throw new Error("请先开始授权。");
    const parsed=parseOAuthCallback(callback);if(!parsed.ok)throw new Error(parsed.reason);
    const {task,id}=enrollment;task.assertOwner();
    const account=await task.mutate<{id:string}>(operations.complete,{path:{upstream_id:enrollment.providerId},body:{id,replace_existing:!!credentialId,callback:parsed.input}});
    setCallback("");
    if(enrollment.endpointId){
      const bindings=await task.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:enrollment.endpointId}});
      if(!bindings.some(row=>row.credential_id===account.id))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:enrollment.endpointId},body:{credential_id:account.id,enabled:true,priority:0,weight:1,concurrency:1}});
    }
    return task.finish();
  },onSuccess:setCompleted});
  const cancel=useMutation({mutationFn:async()=>{
    const enrollment=current.current;
    if(enrollment)await enrollment.task.read(operations.cancel,{path:{upstream_id:enrollment.providerId},body:{id:enrollment.id,replace_existing:!!credentialId},headers:{"If-Match":enrollment.task.revision()}});
  },onSuccess:()=>{setCallback("");onClose();}});
  const busy=start.isPending||complete.isPending||cancel.isPending;
  const close=()=>{if(busy)return;if(completed){useVersionStore.getState().select(completed);onComplete(`${label} 账号授权已保存。`);}else if(complete.isError)onClose();else cancel.mutate();};
  const authorizeUrl=safeExternalUrl(session?.authorization_url);
  return <Sheet title={`${credentialId?"重新授权":"授权"} ${label} 账号`} onEscape={close}>
    <p>{providerName??label}</p>
    {completed?<p role="status">账号授权已保存{current.current?.endpointId?"并连接接口":""}。</p>:!session?<>
      <p>登录 {providerLabel} 账号后，将浏览器跳转的完整回调地址粘贴回来。</p>
      <button disabled={busy||start.isError} onClick={()=>start.mutate()}>开始授权</button>
    </>:session.state==="pending"&&authorizeUrl?<>
      <p><a href={authorizeUrl} target="_blank" rel="noreferrer noopener">打开 {providerLabel} 授权页</a></p>
      <div className="sheet-form"><label>回调地址<textarea rows={3} aria-label="回调地址" value={callback} maxLength={20480} autoComplete="off" spellCheck={false} disabled={busy||complete.isError} onChange={event=>{setCallback(event.target.value);setInputError(undefined);}}/></label></div>
      <p className="muted">授权后，本机回调页可能打不开；复制地址栏中的完整地址即可。</p>
      {inputError?<p role="alert">{inputError}</p>:null}
      <button disabled={busy||!callback.trim()||complete.isError} onClick={()=>{const parsed=parseOAuthCallback(callback);if(!parsed.ok)setInputError(parsed.reason);else complete.mutate();}}>完成授权</button>
    </>:<p role="alert">未能启动授权，请关闭后重新开始。</p>}
    <ConfigurationTaskNotice workingId={workingId} error={start.error??complete.error} onReview={version=>{useVersionStore.getState().select(version);onComplete("请核对已保存的账号修改。");}}/>
    {cancel.isError?<p role="alert">{asAppError(cancel.error).message}</p>:null}
    <div className="sheet-actions"><button className="secondary" disabled={busy} onClick={close}>{completed?"完成":complete.isError?"关闭":"取消"}</button></div>
  </Sheet>;
}
