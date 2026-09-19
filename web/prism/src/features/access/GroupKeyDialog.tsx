import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { resourceOption } from "../../utils/resourceNames";
import { useVersionStore } from "../config-versions/versionStore";
import type { AccessGroupRecord, ClientKeyRecord, IssuedClientKey } from "./model";

type Result=Readonly<{kind:"issued"|"unconfirmed";message:string}>;

/** Advanced signing deliberately writes only to the explicitly selected draft. */
export function GroupKeyDialog({groups,onClose,onSettled}:Readonly<{groups:readonly AccessGroupRecord[];onClose:()=>void;onSettled:()=>void}>){
  const formId="group-key-form";
  const [owner]=useState(()=>useVersionStore.getState());
  const [session]=useState(()=>useSessionStore.getState().generation);
  const [id,setId]=useState("");
  const [groupId,setGroupId]=useState("");
  const [expiry,setExpiry]=useState("");
  const [issued,setIssued]=useState<IssuedClientKey>();
  const [result,setResult]=useState<Result>();
  const [copied,setCopied]=useState(false);
  const [copyError,setCopyError]=useState(false);
  const submitted=useRef(false);
  const stillOwned=()=>{
    const current=useVersionStore.getState();
    return useSessionStore.getState().generation===session&&current.selectionGeneration===owner.selectionGeneration&&current.context?.configVersionId===owner.context?.configVersionId;
  };
  const assertOwner=()=>{if(!stillOwned())throw new CancelledError({silent:true});};
  useEffect(()=>{
    const release=()=>{setIssued(undefined);setResult(undefined);onClose();};
    const stopSession=useSessionStore.subscribe(()=>{if(!stillOwned())release();});
    const stopVersion=useVersionStore.subscribe(()=>{if(!stillOwned())release();});
    return ()=>{stopSession();stopVersion();};
  },[onClose,owner,session]);
  const save=useMutation({gcTime:0,mutationFn:async(input:Readonly<{id:string;groupId:string;expiry:number|null}>):Promise<Result>=>{
    const current=useVersionStore.getState();
    if(owner.context?.status!=="draft"||!stillOwned()||current.context?.revision!==owner.context.revision)throw new Error("草稿已变化，请重新核对后签发。");
    try{
      const key=await call<IssuedClientKey>("issueClientKey",{body:{id:input.id,access_group_id:input.groupId,status:"active",expires_at_ms:input.expiry}},{versionScoped:true,mutating:true});
      assertOwner();
      setIssued(key);
      return {kind:"issued",message:"密钥已保存到当前草稿；发布配置后才会生效。"};
    }catch(cause){
      if(isCancelledError(cause))throw cause;
      assertOwner();
      const kind=asAppError(cause).kind;
      if(kind==="invalid_request"||kind==="conflict"||kind==="session_invalid")throw cause;
      let observed="签发结果未确认。请核对草稿中的密钥记录，不要重复签发。";
      try{
        assertOwner();
        const keys=await call<ClientKeyRecord[]>("listClientKeys",{headers:{"X-Config-Version":owner.context.configVersionId}});
        assertOwner();
        observed=keys.some(key=>key.id===input.id)?"密钥记录已存在，但完整密钥无法再次读取。请撤销该记录并重新签发。":observed;
      }catch(readCause){if(isCancelledError(readCause))throw readCause;assertOwner();}
      assertOwner();
      return {kind:"unconfirmed",message:observed};
    }
  },onSuccess:receipt=>{if(stillOwned())setResult(receipt);else setIssued(undefined);},onError:cause=>{if(!isCancelledError(cause)&&stillOwned())submitted.current=false;}});
  const done=()=>{
    if(save.isPending)return;
    const settled=result!==undefined;
    setIssued(undefined);
    save.reset();
    if(settled)onSettled();else onClose();
  };
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(submitted.current||!id.trim()||!groupId||!stillOwned())return;
    const expiryMs=expiry?new Date(expiry).getTime():null;
    if(expiryMs!==null&&!Number.isFinite(expiryMs))return;
    submitted.current=true;
    save.mutate({id:id.trim(),groupId,expiry:expiryMs});
  };
  const copy=()=>{if(!issued||!stillOwned())return;void navigator.clipboard.writeText(issued.key).then(()=>{setCopied(true);setCopyError(false);},()=>setCopyError(true));};
  return <Sheet title={issued?"Client Key 已签发":result?"签发结果":"按访问组签发"} description={issued?"完整密钥只显示一次；关闭会清除它。":result?"请先核对草稿中的密钥记录。":"高级配置：明确指定 Key 标识和已有访问组；只保存到当前草稿。"} onEscape={done} busy={save.isPending} isDirty={!result&&!issued&&(!!id||!!groupId||!!expiry)} footer={issued?<><button type="button" className="secondary" onClick={copy}>{copied?"已复制":"复制密钥"}</button><SheetDismissButton onDismiss={done} disabled={save.isPending}>完成并清除密钥</SheetDismissButton></>:result?<SheetDismissButton onDismiss={done}>核对草稿</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={save.isPending||submitted.current||!id.trim()||!groupId}>签发到草稿</button></>}>
    {issued?<><p role="status">{result?.message??"密钥已生成，正在确认保存…"}</p><code className="reveal-key mono">{issued.key}</code><p className="muted">请现在复制并安全保存完整密钥。</p>{copyError?<p role="alert">无法访问剪贴板，请手动复制。</p>:null}</>:result?<p role="status">{result.message}</p>:<form id={formId} className="sheet-form" onSubmit={submit}>
      <fieldset disabled={save.isPending||submitted.current}>
        <label>Key ID<input className="mono" required maxLength={128} value={id} onChange={event=>setId(event.target.value)}/></label>
        <label>访问组<select required value={groupId} onChange={event=>setGroupId(event.target.value)}><option value="">请选择访问组</option>{groups.map(group=><option key={group.id} value={group.id}>{resourceOption(group.id,"group",group.name)}</option>)}</select></label>
        <label>过期时间（留空表示永不过期）<input type="datetime-local" value={expiry} onChange={event=>setExpiry(event.target.value)}/></label>
      </fieldset>
    </form>}
    {save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
  </Sheet>;
}
