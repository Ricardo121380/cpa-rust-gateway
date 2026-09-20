import {useEffect,useRef,useState,type FormEvent} from "react";
import {isCancelledError,useQueryClient} from "@tanstack/react-query";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
import {useSessionStore} from "../../session/sessionStore";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";

type Receipt={kind:"acknowledged"|"unconfirmed"|"rejected";message:string;version?:ConfigVersionSummary};
export function DraftCreationDialog({source,versions,onClose,onSelected}:Readonly<{source?:ConfigVersionSummary;versions:readonly ConfigVersionSummary[];onClose:()=>void;onSelected:(version:ConfigVersionSummary)=>void}>) {
 const client=useQueryClient();
 const [id]=useState(()=>`edit-${crypto.randomUUID()}`);
 const [description,setDescription]=useState(source?"编辑服务配置":"新配置草稿");const [parent,setParent]=useState("");
 const [owner]=useState(()=>{const state=useVersionStore.getState();return {session:useSessionStore.getState().generation,selection:state.selectionGeneration,pendingId:state.pending?.id,pendingRevision:state.pending?.revision};});
 const live=useRef(true);const submitted=useRef(false);const [busy,setBusy]=useState(false);const [error,setError]=useState<string>();const [receipt,setReceipt]=useState<Receipt>();const [observed,setObserved]=useState<ConfigVersionSummary>();
 const owned=()=>{const state=useVersionStore.getState();return live.current&&useSessionStore.getState().generation===owner.session&&state.selectionGeneration===owner.selection&&state.pending?.id===owner.pendingId&&state.pending?.revision===owner.pendingRevision;};
 useEffect(()=>{live.current=true;const retire=()=>{if(!owned())onClose();};const a=useVersionStore.subscribe(retire),b=useSessionStore.subscribe(retire);return()=>{live.current=false;a();b();};},[]);
 const submit=async(event:FormEvent)=>{
  event.preventDefault();if(submitted.current||!owned())return;
  if(!description.trim()||new TextEncoder().encode(description.trim()).length>1024){setError("请填写不超过 1024 字节的描述。");return;}
  if(source&&owner.pendingId){setError("已有待应用草稿，请先接续它，不创建第二批配置。");return;}
  setBusy(true);setError(undefined);submitted.current=true;
  let dispatched=false;
  try{
   if(source){
    const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:source.id}});if(!owned())return;
    if(current.id!==source.id||current.status!=="active"||current.revision!==source.revision)throw new Error("活动配置已变化，请重新打开创建操作。");
   }
   if(!owned())return;dispatched=true;
   const version=await call<ConfigVersionSummary>(source?"forkConfigVersion":"createConfigVersion",source?{path:{config_version_id:source.id},headers:{"X-Config-Version":source.id,"If-Match":source.revision},body:{id,description:description.trim()}}:{body:{id,parent_id:parent||null,description:description.trim()}});
   if(!owned())return;
   if(version.id!==id||version.status!=="draft"||(version.parent_id??null)!==(source?.id??(parent||null)))throw new Error("创建回执与目标不一致。");
   void client.invalidateQueries({queryKey:["config-versions"]});
   setReceipt({kind:"acknowledged",message:"草稿创建已确认，尚未应用。",version});
  }catch(cause){
   if(!owned()||isCancelledError(cause))return;const app=asAppError(cause);
   if(!dispatched||["invalid_request","conflict","session_invalid"].includes(app.kind))setReceipt({kind:"rejected",message:`创建未完成：${app.message}。请关闭后重新核对。`});
   else setReceipt({kind:"unconfirmed",message:`创建响应未确认：${app.message}。保留原目标核对，不会创建另一个随机草稿。`});
  }finally{if(owned())setBusy(false);}
 };
 const inspect=async()=>{
  if(!owned()||busy)return;setBusy(true);setError(undefined);
  try{const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:id}});if(!owned())return;if(current.id!==id)throw new Error("返回了另一份配置。");setObserved(current);void client.invalidateQueries({queryKey:["config-versions"]});}
  catch(cause){if(owned()&&!isCancelledError(cause))setError(asAppError(cause).message);}finally{if(owned())setBusy(false);}
 };
 const enter=async(version:ConfigVersionSummary)=>{
  if(!owned()||busy||version.status!=="draft")return;
  setBusy(true);setError(undefined);
  try{
   const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:id}});
   if(!owned())return;
   if(current.id!==id)throw new Error("返回了另一份配置，无法接续。");
   setObserved(current);
   if(current.status!=="draft"||current.revision!==version.revision){
    setError("配置状态或内容已变化，请关闭后从服务端列表重新核对并接续。");return;
   }
   if(!useVersionStore.getState().adoptPending(current,owner)){setError("待应用选择已变化，请重新选择。");return;}
   onSelected(current);
  }catch(cause){if(owned()&&!isCancelledError(cause))setError(asAppError(cause).message);}
  finally{if(owned())setBusy(false);}
 };
 const formId="draft-creation-form";
 return <Sheet title={receipt?"草稿创建结果":source?"编辑当前配置":"创建空草稿"} busy={busy} onEscape={onClose} guardUnsaved={!receipt} footer={receipt?<><SheetDismissButton className="secondary" disabled={busy} onDismiss={onClose}>关闭</SheetDismissButton>{receipt.kind==="acknowledged"&&receipt.version?<button type="button" disabled={busy} onClick={()=>enter(receipt.version!)}>接续草稿</button>:receipt.kind==="unconfirmed"?<><button type="button" className="secondary" disabled={busy} onClick={()=>void inspect()}>核对原草稿</button>{observed?.status==="draft"?<button type="button" disabled={busy} onClick={()=>enter(observed)}>查看并核对草稿</button>:null}</>:null}</>:<><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button type="submit" form={formId} disabled={busy||submitted.current}>创建草稿</button></>}>
  {receipt?<><p role={receipt.kind==="acknowledged"?"status":"alert"}>{receipt.message}</p>{observed?<p>服务端存在这份配置，当前状态：{observed.status}；重读不等于原始创建回执。</p>:null}<details><summary>恢复引用</summary><code>{id}</code></details></>:<form id={formId} className="sheet-form" onSubmit={event=>void submit(event)}><fieldset disabled={busy}><label>描述<input required maxLength={1024} value={description} onChange={event=>setDescription(event.target.value)}/></label>{source?<p>从当前活动配置复制资源；当前服务继续使用原配置。</p>:<><label>谱系来源（可不选）<select value={parent} onChange={event=>setParent(event.target.value)}><option value="">无来源</option>{versions.map(version=><option key={version.id} value={version.id}>{version.description||"未命名配置"} · {version.status}</option>)}</select></label><p>这是空配置。选择谱系来源不会复制资源；需要复制现有资源时请选择“编辑当前配置”。</p></>}{owner.pendingId?<p>新草稿创建后，点击接续才会替换本地待应用选择。原草稿仍保留，不会合并或删除。</p>:null}</fieldset></form>}
  {error?<p role="alert">{error}</p>:null}
 </Sheet>;
}
