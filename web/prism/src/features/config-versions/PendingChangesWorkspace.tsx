import {useInfiniteQuery,useQueryClient} from "@tanstack/react-query";
import {useState} from "react";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {InlineWorkspace} from "../../components/InlineWorkspace";
import {referenceText} from "../../utils/resourceNames";
import {useConfigurationLifecycle} from "./ConfigurationLifecycleHost";
import {useVersionStore,type ConfigVersionSummary} from "./versionStore";
import {assertChangePage,changeLabel,fieldLabel,groupChanges,independentCatalogEvents,type ConfigurationChangePage,type ResourceAuditPage} from "./pendingChanges";
import "./configuration-diff.css";

export function PendingChangesWorkspace({target,active,onClose,onAdopt}:Readonly<{target:ConfigVersionSummary;active?:ConfigVersionSummary;onClose:()=>void;onAdopt:(target:ConfigVersionSummary)=>void}>) {
 const lifecycle=useConfigurationLifecycle();const client=useQueryClient();const context=useVersionStore(state=>state.context);
 const [page,setPage]=useState(0),[auditPage,setAuditPage]=useState(0);
 const key=["pending-changes",target.id,target.revision,active?.id,active?.revision];
 const diff=useInfiniteQuery({queryKey:key,enabled:!!active,initialPageParam:undefined as string|undefined,retry:false,
  queryFn:async({pageParam})=>{
   if(!active)throw new Error("没有活动配置可供比较。");
   const value=await call<ConfigurationChangePage>("compareConfigVersions",{path:{config_version_id:target.id},query:{base_id:active.id,limit:50,...(pageParam?{cursor:pageParam}:{})}});
   assertChangePage(value,active,target);return value;
  },getNextPageParam:(last,pages)=>pages.length<200?last.next_cursor??undefined:undefined});
 const audit=useInfiniteQuery({queryKey:["pending-independent-audit",target.id,target.revision],initialPageParam:undefined as string|undefined,retry:false,
  queryFn:({pageParam})=>call<ResourceAuditPage>("listManagementResourceAuditEvents",{headers:{"X-Config-Version":target.id},query:{limit:100,...(pageParam?{before_id:pageParam}:{})}}),
  getNextPageParam:(last,pages)=>pages.length<100?last.next_before_id??undefined:undefined});
 const rows=diff.data?.pages.flatMap(value=>value.items)??[];
 const complete=!active||(diff.isSuccess&&!diff.isFetching&&!diff.data?.pages.at(-1)?.next_cursor&&!diff.isError);
 const selected=context?.configVersionId===target.id&&context.revision===target.revision;
 const auditEvents=audit.data?.pages.flatMap(value=>independentCatalogEvents(value,target.id))??[];
 const auditPages=Math.max(1,Math.ceil(auditEvents.length/50)),currentAudit=Math.min(auditPage,auditPages-1);
 const auditComplete=audit.isSuccess&&!audit.isError&&!audit.data.pages.at(-1)?.next_before_id;
 const totalPages=Math.max(1,Math.ceil(rows.length/50)),current=Math.min(page,totalPages-1);
 const reset=()=>{setPage(0);void client.invalidateQueries({queryKey:["config-versions"]}).then(()=>client.resetQueries({queryKey:key,exact:true}));};
 const apply=()=>lifecycle.start(target,"publish",{inline:true,proof:{targetRevision:target.revision,baseId:active?.id??null,...(active?{baseRevision:active.revision}:{})}});
 return <InlineWorkspace title="待应用变更" description="核对这一批配置的净变化；全局操作单独列出。" dirty={false} busy={lifecycle.active} onClose={onClose}
  footer={<><button type="button" className="secondary" disabled={lifecycle.active} onClick={onClose}>返回配置</button>{selected?<button type="button" disabled={!complete||lifecycle.active||target.status!=="draft"} onClick={apply}>校验并应用</button>:<button type="button" disabled={lifecycle.active||target.status!=="draft"} onClick={()=>onAdopt(target)}>接续这份草稿</button>}</>}>
  <p>工作草稿：{target.description||"未命名草稿"} · {selected?"当前正在编辑":"只读查看"}</p>
  {!active?<p role="status">当前没有活动配置，无法进行差异比较。可校验并明确应用首份服务配置；此处不推断变化数。</p>:<>
   {diff.isPending?<p role="status">读取配置差异…</p>:null}
   {diff.isError?<p role="alert">差异读取已停止：{asAppError(diff.error).message}。已载入内容仅作旧快照参考，不能用于应用。</p>:null}
   <div className="data-toolbar"><span role="status">{complete?"完整净变化":"已载入部分变化"}：{rows.length} 项</span><button type="button" className="secondary" disabled={diff.isFetching||lifecycle.active} onClick={reset}>重新比较</button></div>
   {rows.length===0&&complete?<p className="empty-state">此草稿相对当前活动配置没有净变化。历史操作记录不等于待应用变化。</p>:null}
   {[...groupChanges(rows.slice(current*50,current*50+50))].map(([group,changes])=><section key={group} className="pending-change-group"><h4>{group}</h4><ul>{changes.map(row=><li key={JSON.stringify([row.resource_kind,row.resource_key])}><strong>{changeLabel[row.change]} · {referenceText(row.resource_key)}</strong><p>{row.changed_fields.map(fieldLabel).join("、")||"资源整体变化"}</p>{row.resource_kind==="credential"?<p className="stat-sub">仅比较凭据存储记录，不判断秘密明文是否相同。</p>:null}<details><summary>精确引用</summary><code>{row.resource_kind} · {row.resource_key}</code></details></li>)}</ul></section>)}
   <nav className="data-toolbar" aria-label="配置差异分页"><button type="button" className="secondary" disabled={current===0} onClick={()=>setPage(current-1)}>上一页</button><span>{current+1} / {totalPages}</span><button type="button" className="secondary" disabled={current+1>=totalPages} onClick={()=>setPage(current+1)}>下一页</button>{diff.hasNextPage?<button type="button" disabled={diff.isFetching||diff.isError} onClick={()=>void diff.fetchNextPage()}>加载下一页差异</button>:null}</nav>
  </>}
  <section aria-label="已独立保存的全局操作"><h4>已独立保存的全局操作</h4><p className="stat-sub">目录按自己的生效时间参与计价；应用、放弃或回滚配置不会删除这些目录，也不会改写历史账本。</p>
   {audit.isPending?<p role="status">读取全局操作证据…</p>:null}{audit.isError?<p role="alert">全局操作证据不可用：{asAppError(audit.error).message}</p>:null}
   {auditEvents.length?<ul>{auditEvents.slice(currentAudit*50,currentAudit*50+50).map(event=><li key={event.id}>{event.action==="billing_catalog_imported"?"价格目录已导入":"恢复目录已新增"} · {new Date(event.occurred_at_ms).toLocaleString()}<details><summary>审计引用</summary><code>{event.id} · {event.resource_id}</code></details></li>)}</ul>:auditComplete?<p>未查到这份草稿关联的目录导入或恢复记录。</p>:null}
   {!auditComplete&&!audit.isPending?<p className="stat-sub">记录尚未完整读取，不能推断没有全局操作。</p>:null}
   {auditPages>1?<nav className="data-toolbar" aria-label="全局操作分页"><button type="button" className="secondary" disabled={currentAudit===0} onClick={()=>setAuditPage(currentAudit-1)}>上一页操作</button><span>{currentAudit+1} / {auditPages}</span><button type="button" className="secondary" disabled={currentAudit+1>=auditPages} onClick={()=>setAuditPage(currentAudit+1)}>下一页操作</button></nav>:null}
   {audit.hasNextPage?<button type="button" className="secondary" disabled={audit.isFetching||audit.isError} onClick={()=>void audit.fetchNextPage()}>加载更多操作证据</button>:null}
   {audit.isError?<button type="button" className="secondary" onClick={()=>void client.resetQueries({queryKey:["pending-independent-audit",target.id,target.revision],exact:true})}>重新读取操作证据</button>:null}
  </section>
 </InlineWorkspace>;
}
