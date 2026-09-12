import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { IdentityDetails } from "../../components/ResourceIdentity";
import { accountName } from "./presentation";
import type { NativeAccount } from "./NativeAccounts";

export type NativeReceipt = Readonly<{account_id:string;revision:number;removed:boolean;runtime_applied:boolean;identity_state?:"observed"|"unavailable"}>;
type Event = Readonly<{id:number;action:string;occurred_at_ms:number}>;
const names = {grok_web:"Grok Web",grok_console:"Grok Console",grok_build:"Grok Build"};
const actions: Record<string,string> = {enabled:"启用账号",disabled:"停用账号",credential_updated:"更新凭据",removed:"移除授权"};

export function NativeAccountDialog({account,onClose,onChanged,onAuthorize}:Readonly<{
  account:NativeAccount;onClose:()=>void;onChanged:(notice:string)=>void;onAuthorize:()=>void;
}>) {
  const [mode,setMode]=useState<"details"|"credential"|"status"|"remove">("details");
  const [error,setError]=useState<string>();
  const [saved,setSaved]=useState<NativeReceipt>();
  const secret=useRef<HTMLTextAreaElement>(null);
  const history=useQuery({queryKey:["native-account-audit",account.id],queryFn:()=>call<Event[]>("listNativeAccountAudit",{path:{account_id:account.id}})});
  const finish=(receipt:NativeReceipt)=>{
    setSaved(receipt);
    if(receipt.runtime_applied) onChanged(receipt.removed?"授权已移除。":receipt.identity_state==="unavailable"?"凭据已更新，渠道暂未返回身份。":"账号修改已应用。");
  };
  const change=useMutation({gcTime:0,mutationFn:async(material?:string)=>{
    const path={account_id:account.id};
    if(mode==="credential") return call<NativeReceipt>("replaceNativeAccountCredential",{path,body:{revision:account.revision,secret:material}});
    if(mode==="remove") return call<NativeReceipt>("deleteNativeAccount",{path,query:{revision:account.revision}});
    return call<NativeReceipt>("updateNativeAccount",{path,body:{revision:account.revision,enabled:!account.enabled}});
  },onSuccess:finish,onError:(error)=>setError(asAppError(error).message),onSettled:():void=>{change.reset();}});
  const apply=useMutation({mutationFn:()=>call<{runtime_applied:boolean}>("applyRuntimeConfiguration"),onSuccess:(result)=>{
    if(result.runtime_applied&&saved)finish({...saved,runtime_applied:true});
    else setError("运行配置尚未应用，请稍后重试。");
  },onError:(error)=>setError(asAppError(error).message)});
  const busy=change.isPending||apply.isPending;
  const select=(value:typeof mode)=>{setMode(value);setError(undefined);};
  const submit=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();setError(undefined);
    const material=secret.current?.value;
    if(secret.current)secret.current.value="";
    change.mutate(material);
  };
  const title=mode==="credential"?"更新 SSO 凭据":mode==="remove"?"移除授权":mode==="status"?account.enabled?"停用账号":"启用账号":"账号详情";
  return <Sheet title={title} layout={mode==="details"?"inspector":"form"} onEscape={()=>!busy&&onClose()}>
    <h3>{accountName(account.identity)??"未提供账号身份"}</h3>
    <p>{names[account.provider]}</p>
    {saved&&!saved.runtime_applied?<div role="alert"><p>修改已保存，运行配置暂未应用。新请求已暂停。</p><button disabled={busy} onClick={()=>apply.mutate()}>应用运行配置</button></div>:mode==="details"?<>
      <StatusBadge status={account.enabled?account.auth_status:"disabled"}>{!account.enabled?"已停用":account.auth_status==="active"?"已保存授权":"需要重新授权"}</StatusBadge>
      <div className="sheet-actions">
        <button onClick={()=>account.provider==="grok_build"?onAuthorize():select("credential")}>{account.provider==="grok_build"?"重新授权":"更新凭据"}</button>
        <button className="secondary" onClick={()=>select("status")}>{account.enabled?"停用":"启用"}</button>
        <button className="secondary" onClick={()=>select("remove")}>移除</button>
      </div>
      <details><summary>最近操作</summary>
        {history.isPending?<p>读取中…</p>:history.isError?<p role="alert">{asAppError(history.error).message}</p>:!history.data?.length?<p className="muted">暂无维护记录</p>:<ul className="account-connections">{history.data.map((entry)=><li key={entry.id}><span>{actions[entry.action]??"账号操作"}</span><time>{new Date(entry.occurred_at_ms).toLocaleString()}</time></li>)}</ul>}
      </details>
      <IdentityDetails entries={[["账号",account.id,accountName(account.identity)??"未提供账号身份"]]}/>
    </>:<form className="sheet-form" onSubmit={submit} autoComplete="off">
      {mode==="credential"?<>
        <label>SSO 凭据<textarea ref={secret} required maxLength={65536} spellCheck={false} autoComplete="off" className="credential-input"/></label>
        <label>选择凭据文件<input type="file" accept=".json,.txt,application/json,text/plain" disabled={busy} onChange={async(event)=>{
          const file=event.currentTarget.files?.[0];event.currentTarget.value="";
          if(!file)return;if(file.size>65536){setError("凭据文件不能超过 64 KiB");return;}
          const target=secret.current;
          try{const value=await file.text();if(target&&target===secret.current)target.value=value;}catch{setError("无法读取文件。");}
        }}/></label>
      </>:<p>{mode==="remove"?"移除此授权，历史请求和费用保留。":account.enabled?"停用后，新请求不再选择此账号。":"启用此账号；认证和额度仍按渠道实际状态判断。"}</p>}
      <div className="sheet-actions"><button type="button" className="secondary" disabled={busy} onClick={()=>select("details")}>返回</button><button disabled={busy}>{mode==="credential"?"保存并应用":"确认"}</button></div>
    </form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
