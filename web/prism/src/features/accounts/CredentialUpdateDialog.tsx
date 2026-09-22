import { isCancelledError, useMutation } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { asAppError } from "../../api/errors";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import type { ManagedCredential } from "./inventory";
import { accountName } from "./presentation";
import { runProviderResourceTask, type ProviderResourceReceipt } from "../upstreams/providerResourceTask";

export function CredentialUpdateDialog({account,onClose,onSaved}:Readonly<{account:ManagedCredential;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const formId="credential-update-form";
  const secret=useRef<HTMLTextAreaElement>(null);
  const submitted=useRef(false);
  const observedStatus=account.credential.status;
  const requiresStatusChoice=observedStatus!=="active"&&observedStatus!=="disabled"&&observedStatus!=="revoked";
  const [statusChoice,setStatusChoice]=useState<"active"|"disabled">();
  const [receipt,setReceipt]=useState<ProviderResourceReceipt>();
  const [validation,setValidation]=useState<string>();
  const save=useMutation({gcTime:0,mutationFn:async()=>{
    let material=secret.current?.value.trim()??"";
    if(secret.current)secret.current.value="";
    try {
      return await runProviderResourceTask("更新账号凭据",async(task,write)=>{
        const current=await task.read<ManagedCredential["credential"]>("getCredential",{path:{credential_id:account.credential.id}});
        if(current.id!==account.credential.id||current.upstream_id!==account.credential.upstream_id||current.kind!==account.credential.kind||current.revision!==account.credential.revision||current.status!==observedStatus)throw new Error("账号授权已变化，请重新读取后操作。");
        const status=requiresStatusChoice?statusChoice:current.status;
        if(status!=="active"&&status!=="disabled"&&status!=="revoked")throw new Error("请选择替换凭据后的账号状态。");
        await write("updateCredential",{path:{credential_id:current.id},body:{id:current.id,kind:current.kind,status,secret:material}});
      });
    } finally {material="";}
  },onSuccess:({receipt:next})=>setReceipt(next),onError:(cause)=>{if(isCancelledError(cause))return;const error=asAppError(cause);if(error.kind==="invalid_request"||error.kind==="conflict")submitted.current=false;}});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;if(requiresStatusChoice&&statusChoice===undefined){setValidation("先选择替换凭据后的账号状态。");return;}const material=secret.current?.value.trim()??"";if(!material){setValidation("请粘贴新的授权资料。");return;}if(material.length>65_536){setValidation("授权资料超出允许长度。");return;}setValidation(undefined);submitted.current=true;save.mutate();};
  const done=()=>{if(receipt)onSaved(receipt.workingVersion);};
  const receiptTitle=receipt?.kind==="saved_applied"?"凭据已保存并应用。":receipt?.kind==="saved_draft"?"凭据已保存到草稿，尚未应用。":receipt?.kind==="saved_unapplied"?"凭据已保存，但应用尚未完成。":"凭据更新结果未确认。";
  return <Sheet title="更新账号凭据" description="替换授权资料，保留现有接口连接。" onEscape={()=>!save.isPending&&(receipt?done():onClose())} busy={save.isPending} footer={receipt?<SheetDismissButton disabled={save.isPending} onDismiss={done}>{receipt.kind==="unconfirmed"?"核对配置":"完成"}</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={submitted.current}>{save.isPending?"正在保存…":"保存并应用"}</button></>}>
    {receipt?<div className="operation-receipt" role="status"><strong>{receiptTitle}</strong><p>{receipt.message}</p><small>完成后将重新读取账号列表。</small></div>:<form id={formId} className="sheet-form" onSubmit={submit}><div className="operation-summary"><span>{account.provider}</span><strong>{accountName(account.identity)??"未提供账号身份"}</strong></div><fieldset className="workflow-section"><legend>授权资料</legend>
      {requiresStatusChoice?<label>替换后的账号状态<select value={statusChoice??""} onChange={(event)=>{setStatusChoice(event.target.value as "active"|"disabled");setValidation(undefined);}} required disabled={submitted.current}><option value="" disabled>选择启用或停用</option><option value="active">替换凭据并启用</option><option value="disabled">替换凭据但保持停用</option></select><small>当前运行状态不能直接写回配置；请明确选择替换后的状态。</small></label>:<p className="muted">当前配置状态会保持不变。</p>}
      <label>新的 API Key 或完整授权文件<textarea ref={secret} required rows={5} maxLength={65536} autoComplete="off" spellCheck={false} disabled={submitted.current}/></label>
      </fieldset>
      {validation?<p role="alert">{validation}</p>:null}
      {save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
    </form>}
  </Sheet>;
}
