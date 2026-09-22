import {useInfiniteQuery,useQueryClient} from "@tanstack/react-query";
import {useState} from "react";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {resourceOption,referenceText} from "../../utils/resourceNames";
import {assertChangePage,changeLabel,fieldLabel,groupChanges,type ConfigurationChangePage} from "./pendingChanges";
import type {ConfigVersionSummary} from "./versionStore";
import "./configuration-diff.css";

/** Historical inspection is read-only and never supplies application proof. */
export function ConfigurationDiff({target,versions,onClose}:Readonly<{target:ConfigVersionSummary;versions:readonly ConfigVersionSummary[];onClose:()=>void}>) {
 const [base,setBase]=useState(versions.some(version=>version.id===target.parent_id)?target.parent_id??"":"");
 const [page,setPage]=useState(0);const client=useQueryClient();
 const baseline=versions.find(version=>version.id===base);
 const key=["configuration-diff",target.id,target.revision,base,baseline?.revision];
 const query=useInfiniteQuery({queryKey:key,enabled:!!baseline,initialPageParam:undefined as string|undefined,retry:false,
  queryFn:async({pageParam})=>{
   if(!baseline)throw new Error("请选择比较基线。");
   const value=await call<ConfigurationChangePage>("compareConfigVersions",{path:{config_version_id:target.id},query:{base_id:baseline.id,limit:50,...(pageParam?{cursor:pageParam}:{})}});
   assertChangePage(value,baseline,target);return value;
  },getNextPageParam:(last,pages)=>pages.length<200?last.next_cursor??undefined:undefined});
 const rows=query.data?.pages.flatMap(value=>value.items)??[];
 const pages=Math.max(1,Math.ceil(rows.length/50)),current=Math.min(page,pages-1);
 const complete=query.isSuccess&&!query.isError&&!query.data.pages.at(-1)?.next_cursor;
 return <section className="configuration-history-review" role="region" aria-label="历史配置差异">
  <header className="page-head"><h3>历史配置差异</h3><button type="button" className="secondary" onClick={onClose}>关闭比较</button></header>
  <div className="data-toolbar"><label>比较基线<select value={base} onChange={event=>{setBase(event.target.value);setPage(0);}}><option value="">请选择比较基线</option>{versions.map(version=><option key={version.id} value={version.id}>{resourceOption(version.id,"config",version.description)} · {version.status}</option>)}</select></label><button type="button" className="secondary" disabled={!baseline||query.isFetching} onClick={()=>{setPage(0);void client.invalidateQueries({queryKey:["config-versions"]}).then(()=>client.resetQueries({queryKey:key,exact:true}));}}>重新比较</button></div>
  <div className="changes-workspace-grid"><div className="changes-workspace-main"><div className="operation-summary"><span>只读历史比较</span><strong>{resourceOption(target.id,"config",target.description)}</strong></div>

  {!baseline?<p>请选择历史基线；不会自动使用不相关的配置。</p>:query.isPending?<p role="status">读取历史差异…</p>:null}
  {query.isError?<p role="alert">比较已停止：{asAppError(query.error).message}。请重新比较；保留的行仅是旧快照。</p>:null}
  {baseline?<p role="status">{complete?"完整差异":"已载入部分差异"}：{rows.length} 项</p>:null}
  {[...groupChanges(rows.slice(current*50,current*50+50))].map(([group,items])=><section className="pending-change-group" key={group}><h4>{group}</h4><ul>{items.map(row=><li key={JSON.stringify([row.resource_kind,row.resource_key])}><strong>{changeLabel[row.change]} · {referenceText(row.resource_key)}</strong><p>{row.changed_fields.map(fieldLabel).join("、")||"资源整体变化"}</p><details><summary>精确引用</summary><code>{row.resource_kind} · {row.resource_key}</code></details></li>)}</ul></section>)}
  {baseline&&complete&&rows.length===0?<p className="empty-state">两个版本的资源记录没有差异。</p>:null}
  <nav className="data-toolbar" aria-label="历史差异分页"><button type="button" className="secondary" disabled={current===0} onClick={()=>setPage(current-1)}>上一页</button><span>{current+1} / {pages}</span><button type="button" className="secondary" disabled={current+1>=pages} onClick={()=>setPage(current+1)}>下一页</button>{query.hasNextPage?<button type="button" disabled={query.isFetching||query.isError} onClick={()=>void query.fetchNextPage()}>加载更多差异</button>:null}</nav>
 </div><aside className="changes-workspace-aside"><section className="changes-impact"><h4>比较范围</h4><p>仅展示存储记录及变化字段名；凭据差异不代表已比较秘密明文。</p><p>运行状态和全局价格目录不属于配置差异。历史比较不能作为应用依据。</p></section></aside></div>
 </section>;
}
