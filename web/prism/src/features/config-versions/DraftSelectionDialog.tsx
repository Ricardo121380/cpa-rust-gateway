import {useEffect,useRef,useState} from "react";
import {isCancelledError} from "@tanstack/react-query";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
import {useSessionStore} from "../../session/sessionStore";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";

/** An explicit, read-only adoption decision; no draft is created or merged here. */
export function DraftSelectionDialog({id,onClose,onSelected}:Readonly<{id:string;onClose:()=>void;onSelected:(summary:ConfigVersionSummary)=>void}>) {
 const [owner]=useState(()=>{const state=useVersionStore.getState();return {session:useSessionStore.getState().generation,selection:state.selectionGeneration,pendingId:state.pending?.id,pendingRevision:state.pending?.revision};});
 const live=useRef(true);const readGeneration=useRef(0);const [busy,setBusy]=useState(true);const [target,setTarget]=useState<ConfigVersionSummary>();const [error,setError]=useState<string>();const [missing,setMissing]=useState(false);
 const owned=()=>{const state=useVersionStore.getState();return live.current&&useSessionStore.getState().generation===owner.session&&state.selectionGeneration===owner.selection&&state.pending?.id===owner.pendingId&&state.pending?.revision===owner.pendingRevision;};
 const read=async()=>{
  if(!owned())return;const generation=++readGeneration.current;setBusy(true);setError(undefined);setMissing(false);
  try{const value=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:id}});if(!owned()||generation!==readGeneration.current)return;if(value.id!==id)throw new Error("读取返回了另一份配置。");setTarget(value);}
  catch(cause){if(owned()&&generation===readGeneration.current&&!isCancelledError(cause)){const app=asAppError(cause);setMissing(app.status===404&&app.code==="management_resource_not_found");setError(app.message);}}
  finally{if(owned()&&generation===readGeneration.current)setBusy(false);}
 };
 useEffect(()=>{
  live.current=true;void read();
  const retire=()=>{if(!owned())onClose();};const a=useVersionStore.subscribe(retire),b=useSessionStore.subscribe(retire);
  return()=>{live.current=false;readGeneration.current++;a();b();};
 },[]);
 const accept=async()=>{
  if(!owned()||!target||busy)return;setBusy(true);setError(undefined);
  try{
   const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:id}});if(!owned())return;
   if(current.id!==id)throw new Error("读取返回了另一份配置，无法接续。");
   if(current.revision!==target.revision||current.status!==target.status){setTarget(current);setError("配置在确认期间发生变化，请核对新状态后再继续。");return;}
   const store=useVersionStore.getState();
   const changed=current.status==="draft"?store.adoptPending(current,owner):store.resolvePending(current,owner);
   if(!changed){setError("待应用选择已改变，请关闭后重新选择。");return;}
   onSelected(current);
  }catch(cause){if(owned()&&!isCancelledError(cause))setError(asAppError(cause).message);}
  finally{if(owned())setBusy(false);}
 };
 const replacing=owner.pendingId!==undefined&&owner.pendingId!==id;
 return <Sheet title="接续配置草稿" layout="confirm" busy={busy} onEscape={onClose} guardUnsaved={false} footer={<><SheetDismissButton className="secondary" disabled={busy} onDismiss={onClose}>取消</SheetDismissButton>{target&&!missing?<button type="button" disabled={busy} onClick={()=>void accept()}>{target.status==="draft"?(replacing?"切换待应用选择":"接续草稿"):"查看已观测配置"}</button>:<button type="button" className="secondary" disabled={busy} onClick={()=>void read()}>重新读取</button>}</>}>
  {busy?<p role="status">核对服务端配置状态…</p>:null}
  {target?<><p>{target.description||"未命名配置"} · {target.status==="draft"?"草稿":target.status==="active"?"当前读取为已发布":"已归档"}</p>{target.status!=="draft"?<p>这份待应用配置已由其他操作发布或归档；本页面没有执行应用。继续后更新本地选择。</p>:replacing?<p>只替换本页面的待应用选择。原草稿仍保存在服务端，不会合并、删除、回滚或发布。</p>:<p>进入这份草稿后重新读取资源与变更，不沿用之前的校验结果。</p>}</>:null}
  {missing?<p role="alert">服务端确认找不到这份配置；原待应用记录保留，未自动选择其他草稿。</p>:error?<p role="alert">{error}</p>:null}
 </Sheet>;
}
