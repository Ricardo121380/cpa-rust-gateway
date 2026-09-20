import {useEffect,useState} from "react";
import {useQuery,useQueryClient} from "@tanstack/react-query";
import {useSearchParams} from "react-router-dom";
import {call} from "../../api/client";
import {ReadStatus} from "../../components/ReadStatus";
import {ResourceIdentity,IdentityDetails} from "../../components/ResourceIdentity";
import {useOperationBoundary} from "../../components/OperationBoundary";
import {useMessages} from "../../i18n/messages";
import {ConfigurationDiff} from "./ConfigurationDiff";
import {useConfigurationLifecycle} from "./ConfigurationLifecycleHost";
import {PendingChangesWorkspace} from "./PendingChangesWorkspace";
import {DraftSelectionDialog} from "./DraftSelectionDialog";
import {DraftCreationDialog} from "./DraftCreationDialog";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";

type Dialog={kind:"adopt";id:string}|{kind:"create";source?:ConfigVersionSummary};
const formatTime=(ms:number)=>new Date(ms).toLocaleString();
export function VersionsPage(){
 const t=useMessages();const client=useQueryClient();const admission=useOperationBoundary();const lifecycle=useConfigurationLifecycle();
 const context=useVersionStore(state=>state.context),pending=useVersionStore(state=>state.pending);
 const [params,setParams]=useSearchParams();const [dialog,setDialog]=useState<Dialog>();
 const [collection,setCollection]=useState<"draft"|"archived">("draft");const [inspection,setInspection]=useState<ConfigVersionSummary>();
 const versions=useQuery({queryKey:["config-versions"],queryFn:()=>call<ConfigVersionSummary[]>("listConfigVersions"),staleTime:0,refetchOnMount:"always",retry:false});
 const active=versions.data?.find(version=>version.status==="active");
 const reviewId=params.get("review"),resumeId=params.get("resume");
 const target=versions.data?.find(version=>version.id===reviewId);
 useEffect(()=>{if(resumeId)setDialog({kind:"adopt",id:resumeId});},[resumeId]);
 const removeParams=(...keys:string[])=>{const next=new URLSearchParams(params);for(const key of keys)next.delete(key);setParams(next);};
 const closeDialog=()=>{setDialog(undefined);if(resumeId)removeParams("resume");};
 const review=(id:string)=>admission.request(()=>{setInspection(undefined);setParams({review:id});});
 const adopt=(version:ConfigVersionSummary)=>admission.request(()=>setDialog({kind:"adopt",id:version.id}));
 const selected=(version:ConfigVersionSummary)=>{
  setDialog(undefined);void client.invalidateQueries({queryKey:["config-versions"]});
  if(version.status==="draft")setParams({review:version.id});else setParams({});
 };
 const begin=()=>admission.request(()=>{
  if(pending){setDialog({kind:"adopt",id:pending.id});return;}
  if(context?.status==="draft"){setDialog({kind:"adopt",id:context.configVersionId});return;}
  setDialog({kind:"create",...(active?{source:active}:{})});
 });
 const view=(version:ConfigVersionSummary)=>admission.request(()=>{useVersionStore.getState().select(version);setParams({});});
 const inspect=(version:ConfigVersionSummary)=>admission.request(()=>{removeParams("review","intent");setInspection(version);});
 return <section className="versions-page">
  <header className="page-head"><div><h2>{t.nav.versions}</h2><p className="page-subtitle">接续一份草稿，核对完整变更后统一应用。</p></div><div className="page-actions">
   <button type="button" disabled={lifecycle.active||versions.isFetching} onClick={begin}>{pending?"接续待应用修改":"编辑当前配置"}</button>
   <button type="button" className="secondary" disabled={lifecycle.active||versions.isFetching} onClick={()=>admission.request(()=>setDialog({kind:"create"}))}>创建空草稿</button>
   <button type="button" className="secondary" disabled={lifecycle.active||!active||context?.configVersionId!==active.id} onClick={()=>active&&lifecycle.start(active,"rollback")}>回滚到上一版本</button>
  </div></header>
  <ReadStatus pending={versions.isPending} error={versions.error} hasData={versions.data!==undefined} retry={()=>void versions.refetch()}/>
  <button type="button" className="secondary" disabled={versions.isFetching||lifecycle.active} onClick={()=>void versions.refetch()}>重新读取配置</button>
  {reviewId&&target?.status==="draft"?<PendingChangesWorkspace key={`${target.id}:${target.revision}`} target={target} active={active} onClose={()=>removeParams("review","intent")} onAdopt={adopt}/>:reviewId&&!versions.isPending?<p role="alert">{target?"此配置不再是草稿，请重新核对服务端状态。":"未读取到所选草稿；不会自动替换为另一份配置。"}</p>:null}
  {inspection?<ConfigurationDiff target={versions.data?.find(version=>version.id===inspection.id)??inspection} versions={versions.data??[]} onClose={()=>setInspection(undefined)}/>:null}
  {active?<article className="card published-configuration" data-version-id={active.id}><div><span className="configuration-state">已发布配置</span><h3><ResourceIdentity id={active.id} name={active.description} kind="config"/></h3><p className="entity-meta">创建于 {formatTime(active.created_at_ms)}</p><IdentityDetails entries={[["配置 ID",active.id],["修订号",active.revision],["来源 ID",active.parent_id??"无"]]}/><details className="reading-notes"><summary>配置与程序版本有什么区别？</summary><p>查看配置不影响流量。确认应用后，新请求使用新的运行配置，已开始的请求继续完成；更新程序版本属于部署。</p></details></div><div className="row-actions"><button className="secondary" onClick={()=>inspect(active)}>查看历史差异</button><button disabled={context?.configVersionId===active.id||lifecycle.active} onClick={()=>view(active)}>{context?.configVersionId===active.id?"正在查看":"查看已发布配置"}</button></div></article>:null}
  <div className="configuration-tabs" role="group" aria-label="配置集合"><button className="secondary" aria-pressed={collection==="draft"} onClick={()=>setCollection("draft")}>草稿 · {versions.data?.filter(version=>version.status==="draft").length??"…"}</button><button className="secondary" aria-pressed={collection==="archived"} onClick={()=>setCollection("archived")}>历史 · {versions.data?.filter(version=>version.status==="archived").length??"…"}</button></div>
  <div className="data-panel configuration-list"><div className="tablewrap"><table className="responsive-table"><thead><tr><th>配置</th><th>创建时间</th><th>操作</th></tr></thead><tbody>{versions.data?.filter(version=>version.status===collection).map(version=>{
   const isSelected=context?.configVersionId===version.id;
   return <tr key={version.id} data-selected={isSelected} data-version-id={version.id}><td data-label="配置"><ResourceIdentity id={version.id} name={version.description} kind="config"/>{pending?.id===version.id?<p className="stat-sub">当前待应用批次</p>:null}<IdentityDetails entries={[["配置 ID",version.id],["修订号",version.revision],["来源 ID",version.parent_id??"无"]]}/></td><td data-label="创建时间">{formatTime(version.created_at_ms)}</td><td data-label="操作"><div className="row-actions">
    <button className="secondary" disabled={lifecycle.active} onClick={()=>version.status==="draft"?review(version.id):inspect(version)}>{version.status==="draft"?"查看变更":"查看历史差异"}</button>
    <button disabled={isSelected||lifecycle.active} onClick={()=>version.status==="draft"?adopt(version):view(version)}>{isSelected?"正在查看":version.status==="draft"?"编辑草稿":"查看历史"}</button>
    <button className="secondary" disabled={!isSelected||lifecycle.active} onClick={()=>lifecycle.start(version,version.status==="active"?"rollback":"publish",{validateOnly:true})}>验证</button>
    {version.status==="draft"?<button disabled={!isSelected||lifecycle.active} onClick={()=>review(version.id)}>校验并应用</button>:null}
   </div></td></tr>;
  })}</tbody></table></div>{versions.isSuccess&&!versions.data.some(version=>version.status===collection)?<p className="empty-state">{collection==="draft"?"没有草稿；不会自动创建或选择新的配置。":"暂无历史配置。"}</p>:null}</div>
  {dialog?.kind==="adopt"?<DraftSelectionDialog key={dialog.id} id={dialog.id} onClose={closeDialog} onSelected={selected}/>:dialog?.kind==="create"?<DraftCreationDialog source={dialog.source} versions={versions.data??[]} onClose={closeDialog} onSelected={selected}/>:null}
 </section>;
}
