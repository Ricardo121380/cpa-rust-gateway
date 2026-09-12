import { useMutation } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Sheet } from "../../components/Sheet";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import type { ManagedCredential } from "./inventory";
import { accountName } from "./presentation";

export function CredentialUpdateDialog({account,onClose,onSaved}:Readonly<{account:ManagedCredential;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const secret=useRef<HTMLTextAreaElement>(null);
  const submitted=useRef(false);
  const [workingId,setWorkingId]=useState<string>();
  const save=useMutation({gcTime:0,mutationFn:async()=>{
    let material=secret.current?.value.trim()??"";
    if(secret.current)secret.current.value="";
    try {
      const task=await beginConfigurationTask("更新账号凭据");setWorkingId(task.version.id);
      const current=await task.read<ManagedCredential["credential"]>("getCredential",{path:{credential_id:account.credential.id}});
      if(current.revision!==account.credential.revision)throw new Error("账号授权已变化，请重新读取后操作。");
      await task.mutate("updateCredential",{path:{credential_id:current.id},body:{id:current.id,kind:current.kind,status:current.status,secret:material}});
      material="";
      return task.finish();
    } finally {material="";}
  },onSuccess:onSaved});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  return <Sheet title="更新凭据" onEscape={()=>!save.isPending&&onClose()}><h3>{accountName(account.identity)??account.provider}</h3><form className="sheet-form" onSubmit={submit}>
    <label>新的 API Key 或完整授权文件<textarea ref={secret} required rows={5} maxLength={65536} autoComplete="off" spellCheck={false} disabled={submitted.current}/></label>
    <p className="muted">保留账号当前的启停状态与接口连接。</p>
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
    <div className="sheet-actions"><button type="button" className="secondary" disabled={save.isPending} onClick={onClose}>取消</button><button type="submit" disabled={submitted.current}>{save.isPending?"正在保存…":"保存并应用"}</button></div>
  </form></Sheet>;
}
