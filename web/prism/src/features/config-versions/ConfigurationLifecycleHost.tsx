import {createContext,useContext,useEffect,useRef,useState,type ReactNode} from "react";
import {isCancelledError,useQueryClient} from "@tanstack/react-query";
import {useNavigate} from "react-router-dom";
import {asAppError} from "../../api/errors";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
import {ResourceIdentity} from "../../components/ResourceIdentity";
import {useOperationBoundary} from "../../components/OperationBoundary";
import {useSessionStore} from "../../session/sessionStore";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";
import {captureLifecycleOwner,isLifecycleOwner,prepareConfigurationLifecycle,commitConfigurationLifecycle,observeConfigurationLifecycle,type LifecycleMode,type LifecycleOwner,type LifecycleReceipt,type LifecycleObservation,type PreparedLifecycle,type ReviewProof} from "./configurationLifecycle";

type Attempt=Readonly<{owner:LifecycleOwner;mode:LifecycleMode;inline:boolean;validateOnly:boolean}>;
type Panel=
 |{kind:"preparing";attempt:Attempt}
 |{kind:"prepared";attempt:Attempt;prepared:PreparedLifecycle}
 |{kind:"writing";attempt:Attempt;prepared:PreparedLifecycle}
 |{kind:"failed";attempt:Attempt;message:string}
 |{kind:"receipt";attempt:Attempt;receipt:LifecycleReceipt;observation?:LifecycleObservation;readError?:string;reading:boolean};
type API=Readonly<{active:boolean;openSelected:(version:ConfigVersionSummary)=>void;start:(source:ConfigVersionSummary,mode:LifecycleMode,options?:{inline?:boolean;validateOnly?:boolean;proof?:ReviewProof})=>void}>;
const Context=createContext<API | undefined>(undefined);
export function useConfigurationLifecycle(){const value=useContext(Context);if(!value)throw new Error("Configuration lifecycle host missing");return value;}

/** The shell owns exactly one lifecycle attempt, independent of outlet remounts. */
export function ConfigurationLifecycleHost({children}:{children:ReactNode}){
 const admission=useOperationBoundary();const queries=useQueryClient();const navigate=useNavigate();
 const [selectedDestination,setSelectedDestination]=useState<ConfigVersionSummary>();
 useEffect(()=>{if(selectedDestination){if(useVersionStore.getState().context?.configVersionId===selectedDestination.id)void navigate(selectedDestination.status==="draft"?`/versions?review=${encodeURIComponent(selectedDestination.id)}`:"/versions");setSelectedDestination(undefined);}},[selectedDestination,navigate]);
 const [panel,setPanel]=useState<Panel>();const current=useRef<Attempt | undefined>(undefined);
 const [entryError,setEntryError]=useState<string>();
 const alive=(attempt:Attempt)=>current.current===attempt&&isLifecycleOwner(attempt.owner);
 const close=()=>{current.current=undefined;setPanel(undefined);};
 useEffect(()=>{
  const retire=()=>{if(current.current&&!isLifecycleOwner(current.current.owner)){current.current=undefined;setPanel(undefined);setEntryError(undefined);}};
  const a=useVersionStore.subscribe(retire),b=useSessionStore.subscribe(retire);return()=>{a();b();current.current=undefined;};
 },[]);
 const start:API["start"]=(source,mode,options={})=>{
  const begin=()=>{
   if(current.current)return;
   let owner:LifecycleOwner;
   try{owner=captureLifecycleOwner(source);}catch(cause){setEntryError(asAppError(cause).message);return;}
   const attempt={owner,mode,inline:options.inline??false,validateOnly:options.validateOnly??false};
   current.current=attempt;setEntryError(undefined);setPanel({kind:"preparing",attempt});
   void prepareConfigurationLifecycle(owner,mode,options.proof,options.validateOnly).then(prepared=>{if(alive(attempt))setPanel({kind:"prepared",attempt,prepared});}).catch(cause=>{if(alive(attempt)&&!isCancelledError(cause))setPanel({kind:"failed",attempt,message:asAppError(cause).message});});
  };
  if(options.inline)begin();else admission.request(begin);
 };
 const observe=(attempt:Attempt,receipt:LifecycleReceipt)=>{
  if(!alive(attempt))return;
  setPanel(previous=>previous?.kind==="receipt"&&previous.attempt===attempt?{...previous,reading:true,readError:undefined}:previous);
  void observeConfigurationLifecycle(receipt).then(observation=>{
   if(alive(attempt))setPanel(previous=>previous?.kind==="receipt"&&previous.attempt===attempt?{...previous,observation,reading:false}:previous);
  }).catch(cause=>{if(alive(attempt)&&!isCancelledError(cause))setPanel(previous=>previous?.kind==="receipt"&&previous.attempt===attempt?{...previous,reading:false,readError:asAppError(cause).message}:previous);});
 };
 const commit=(prepared:PreparedLifecycle,attempt:Attempt)=>{
  if(!alive(attempt)||panel?.kind!=="prepared")return;
  setPanel({kind:"writing",attempt,prepared});
  void commitConfigurationLifecycle(prepared).then(receipt=>{
   if(!alive(attempt))return;
   setPanel({kind:"receipt",attempt,receipt,reading:false});
   void queries.invalidateQueries({queryKey:["config-versions"]});
   if(receipt.kind==="acknowledged")observe(attempt,receipt);
  }).catch(cause=>{if(alive(attempt)&&!isCancelledError(cause))setPanel({kind:"failed",attempt,message:asAppError(cause).message});});
 };
 const finish=()=>{
  const next=panel?.kind==="receipt"&&panel.receipt.kind==="acknowledged"&&panel.observation?.target.status==="active"&&panel.observation.activeId===panel.observation.target.id?panel.observation.target:undefined;
  close();if(next)useVersionStore.getState().select(next);
 };
 const busy=panel?.kind==="preparing"||panel?.kind==="writing"||panel?.kind==="receipt"&&panel.reading;
 const title=panel?.kind==="preparing"?"正在校验配置":panel?.kind==="writing"?"正在应用配置":panel?.kind==="failed"?"配置操作未完成":panel?.kind==="receipt"?"配置操作结果":panel?.attempt.validateOnly?"验证结果":panel?.attempt.mode==="rollback"?"确认回滚":"确认应用配置";
 return <Context.Provider value={{active:panel!==undefined,start,openSelected:setSelectedDestination}}>{children}
  {entryError?<div role="alert" className="conflict-bar">{entryError}<button type="button" onClick={()=>setEntryError(undefined)}>关闭</button></div>:null}
  {panel?<Sheet navigationOwned={panel.attempt.inline} title={title} layout={panel.kind==="prepared"&&!panel.attempt.validateOnly?"confirm":"inspector"} tone={panel.attempt.mode==="rollback"?"danger":"default"} busy={busy} onEscape={finish} guardUnsaved={false}
   footer={panel.kind==="prepared"&&!panel.attempt.validateOnly?<><SheetDismissButton className="secondary" onDismiss={close}>取消</SheetDismissButton><button type="button" className={panel.attempt.mode==="rollback"?"danger":undefined} onClick={()=>commit(panel.prepared,panel.attempt)}>{panel.attempt.mode==="rollback"?"确认回滚":"确认应用"}</button></>:panel.kind==="receipt"?<><button type="button" className="secondary" disabled={busy} onClick={()=>observe(panel.attempt,panel.receipt)}>核对服务端状态</button><SheetDismissButton disabled={busy} onDismiss={finish}>{panel.receipt.kind==="acknowledged"?"完成":"返回核对"}</SheetDismissButton></>:<SheetDismissButton disabled={busy} onDismiss={close}>{busy?"处理中…":"关闭"}</SheetDismissButton>}>
   <p>配置：<ResourceIdentity id={panel.attempt.owner.source.id} kind="config" name={panel.attempt.owner.source.description}/></p>
   {panel.kind==="preparing"||panel.kind==="writing"?<p role="status">{panel.kind==="preparing"?"正在核对版本并校验，请稍候…":"正在提交已确认的配置，请稍候…"}</p>:null}
   {panel.kind==="prepared"?<><p role="status">此修订的配置校验通过。</p><p>{panel.attempt.validateOnly?"校验不会应用配置，也不保留未来的应用权限。":"确认后新请求使用这份配置；已开始的请求继续完成。"}</p>{panel.attempt.mode==="rollback"?<p>回滚目标：<ResourceIdentity id={panel.prepared.target.id} kind="config" name={panel.prepared.target.description}/></p>:null}<p className="stat-sub">全局价格目录、历史账本和原生账号运行状态不随配置回滚。</p></>:null}
   {panel.kind==="failed"?<p role="alert">{panel.message}</p>:null}
   {panel.kind==="receipt"?<><p role={panel.receipt.kind==="acknowledged"?"status":"alert"}>{panel.receipt.message}</p>{panel.reading?<p role="status">正在读取服务端状态…</p>:null}{panel.readError?<p role="alert">回执保留，状态重读失败：{panel.readError}</p>:null}{panel.observation?<p>{panel.observation.target.status==="active"&&panel.observation.activeId===panel.observation.target.id?(panel.receipt.kind==="acknowledged"?"已核对：目标仍为当前活动配置。":"当前读取到目标处于活动状态；这不证明本次请求已获确认。"):"目标已被后续操作替换或仍未应用；不会将它当作当前活动配置。"}</p>:null}</>:null}
  </Sheet>:null}
 </Context.Provider>;
}
