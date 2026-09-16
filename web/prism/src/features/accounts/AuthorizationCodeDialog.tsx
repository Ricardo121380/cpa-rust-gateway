import {useMutation} from "@tanstack/react-query";
import {useRef,useState,type FormEvent} from "react";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
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
  const formId=`${channel}-authorization-callback`;
  const callbackErrorId=`${formId}-error`;
  const providerLabel=channel==="claude"?"Claude":"OpenAI";
  const operations=channel==="claude"?{start:"startClaudeEnrollment",complete:"completeClaudeEnrollment",cancel:"cancelClaudeEnrollment"} as const:{start:"startCodexEnrollment",complete:"completeCodexEnrollment",cancel:"cancelCodexEnrollment"} as const;
  const current=useRef<Enrollment|undefined>(undefined);
  const [session,setSession]=useState<Session>();
  const [callback,setCallback]=useState("");
  const [inputError,setInputError]=useState<string>();
  const [workingId,setWorkingId]=useState<string>();
  const [completed,setCompleted]=useState<ConfigVersionSummary>();
  const [completionError,setCompletionError]=useState<unknown>();
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
    try {
      if(enrollment.endpointId){
        const bindings=await task.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:enrollment.endpointId}});
        if(!bindings.some(row=>row.credential_id===account.id))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:enrollment.endpointId},body:{credential_id:account.id,enabled:true,priority:0,weight:1,concurrency:1}});
      }
      return await task.finish();
    } catch (error) {
      // Completion consumed the callback and persisted the credential. An
      // application/binding failure is recoverable configuration work, never
      // a reason to resubmit the callback or cancel the consumed enrollment.
      setCompletionError(error);
      return undefined;
    }
  },onSuccess:(version)=>{if(version!==undefined)setCompleted(version);}});
  const cancel=useMutation({mutationFn:async()=>{
    const enrollment=current.current;
    if(enrollment)await enrollment.task.read(operations.cancel,{path:{upstream_id:enrollment.providerId},body:{id:enrollment.id,replace_existing:!!credentialId},headers:{"If-Match":enrollment.task.revision()}});
  }});
  const busy=start.isPending||complete.isPending||cancel.isPending;
  const dismissAuthorization=async()=>{
    // The callback is transient material. Clear it before cancellation so a
    // failed close never leaves it behind a secondary error state.
    setCallback("");
    if(completed||completionError!==undefined)return true;
    if(session?.state!=="pending")return true;
    try {await cancel.mutateAsync();return true;} catch {return false;}
  };
  const close=()=>{setCallback("");if(completed){useVersionStore.getState().select(completed);onComplete(`${label} 账号授权已保存。`);}else if(completionError!==undefined)onComplete(`${label} 账号授权已保存，配置尚未应用，请核对待应用的修改。`);else onClose();};
  const authorizeUrl=safeExternalUrl(session?.authorization_url);
  const submitCallback=(event:FormEvent<HTMLFormElement>)=>{event.preventDefault();const parsed=parseOAuthCallback(callback);if(!parsed.ok){setInputError(parsed.reason);return;}complete.mutate();};
  const footer=completed||completionError!==undefined?<SheetDismissButton disabled={busy}>完成</SheetDismissButton>:!session?<><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button type="button" disabled={busy||start.isError} onClick={()=>start.mutate()}>开始授权</button></>:session.state==="pending"&&authorizeUrl&&!complete.isError?<><SheetDismissButton className="secondary" disabled={busy}>取消授权</SheetDismissButton><button type="submit" form={formId} disabled={busy||!callback.trim()}>完成授权</button></>:<SheetDismissButton disabled={busy}>关闭</SheetDismissButton>;
  return <Sheet title={`${credentialId?"重新授权":"授权"} ${label} 账号`} description={credentialId?"为当前账号更新授权，不会新建连接或改变已配置的接口。":"在官方页面完成登录后，粘贴浏览器带回的完整回调地址。"} onEscape={close} onBeforeDismiss={dismissAuthorization} busy={busy} blockNavigation={session?.state==="pending"&&completed===undefined&&completionError===undefined} footer={footer}>
    {providerName&&providerName!==label?<p className="muted">{providerName}</p>:null}
    {completed||completionError!==undefined?<p role="status">{completionError===undefined?`账号授权已保存${current.current?.endpointId?"并连接接口":""}。`:"账号授权已保存；接口连接或配置尚未应用。"}</p>:!session?<>
      <p>登录 {providerLabel} 账号后，将浏览器跳转的完整回调地址粘贴回来。</p>
    </>:session.state==="pending"&&authorizeUrl?<>
      <p><a href={authorizeUrl} target="_blank" rel="noreferrer noopener">打开 {providerLabel} 授权页</a></p>
      <form id={formId} className="sheet-form" onSubmit={submitCallback}><label>回调地址<textarea rows={3} aria-label="回调地址" value={callback} maxLength={20480} autoComplete="off" spellCheck={false} aria-invalid={inputError===undefined?undefined:true} aria-describedby={inputError===undefined?undefined:callbackErrorId} disabled={busy||complete.isError} onChange={event=>{setCallback(event.target.value);setInputError(undefined);}}/></label></form>
      <p className="muted">授权后，本机回调页可能打不开；复制地址栏中的完整地址即可。</p>
      {inputError?<p id={callbackErrorId} role="alert">{inputError}</p>:null}
    </>:<p role="alert">未能启动授权，请关闭后重新开始。</p>}
    <ConfigurationTaskNotice workingId={workingId} error={completionError??start.error??complete.error} onReview={version=>{useVersionStore.getState().select(version);onComplete("请核对已保存的账号修改。");}}/>
    {cancel.isError?<p role="alert">{asAppError(cancel.error).message}</p>:null}
  </Sheet>;
}
