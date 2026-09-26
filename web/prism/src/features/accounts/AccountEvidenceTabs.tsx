import {useState,type ReactNode} from "react";
import {useQuery} from "@tanstack/react-query";
import {Link} from "react-router-dom";
import {call} from "../../api/client";
import {PagedReadStatus} from "../../components/PagedReadStatus";
import {ResourceIdentity} from "../../components/ResourceIdentity";
import {useVersionStore} from "../config-versions/versionStore";
import {EntitlementEvidence} from "../runtime/EntitlementEvidence";
import {authStatusMeta,runtimeStatusMeta,freshnessMeta,formatObservedAt,type PoolAccount,type PoolSnapshot,type CatalogRow} from "../runtime/model";

const tabs=[['overview','概览'],['quota','额度'],['configuration','配置'],['models','模型'],['diagnostics','诊断']] as const;
export function AccountEvidenceTabs({accountId,overview,configuration,onNavigate}:{accountId:string;overview:ReactNode;configuration:ReactNode;onNavigate:()=>void}) {
 const [tab,setTab]=useState<(typeof tabs)[number][0]>('overview');
 const scope=useVersionStore(s=>s.context?.configVersionId);
 const runtime=useQuery({queryKey:['account-evidence',scope,accountId],enabled:!!scope&&(tab==='quota'||tab==='diagnostics'),retry:false,queryFn:async({signal})=>{
  const rows:PoolAccount[]=[];let cursor:string|undefined;let count=0;let snapshot:string|undefined;let observedAt:number|undefined;
  do {const page=await call<PoolSnapshot>('listProviderAccountPools',{query:{limit:100,...(cursor?{cursor}:{})},signal});if(snapshot&&snapshot!==page.snapshot_id)throw new Error('运行快照已改变，请重新读取');snapshot=page.snapshot_id;observedAt=page.observed_at_ms;rows.push(...page.items.filter(row=>row.account_id===accountId));count+=page.items.length;cursor=page.next_cursor??undefined;if(cursor&&count>=10000)throw new Error('运行目录超出本次读取范围，请在运行状态中按渠道筛选');}while(cursor);return {items:rows,observedAt};
 }});
 const catalog=useQuery({queryKey:['catalog-status',scope],enabled:!!scope&&tab==='models',retry:false,queryFn:({signal})=>call<CatalogRow[]>('getCatalogStatus',{signal},{versionScoped:true})});
 const observed=(catalog.data??[]).filter(row=>row.credential_id===accountId);
 return <div className="account-evidence"><nav className="workspace-navigation" aria-label="账号详情分类">{tabs.map(([value,label])=><button type="button" key={value} aria-pressed={tab===value} onClick={()=>setTab(value)}>{label}</button>)}</nav>
  <div className="account-evidence-body">{tab==='overview'?overview:tab==='configuration'?configuration:tab==='models'?<>
   <PagedReadStatus query={catalog}/>{catalog.data===undefined?null:!observed.length?<p>此账号尚无成功目录观测；不据套餐推断可用模型。</p>:observed.map(row=><section key={row.endpoint_id} className="account-evidence-card"><ResourceIdentity id={row.endpoint_id} kind="endpoint"/><p>{freshnessMeta(row.freshness).label} · 上游模型 {row.model_count??'未观测'}</p><p>观测时间：{row.freshness==='missing'?'未观测':formatObservedAt(row.observed_at_ms)}</p><Link to={`/catalog?${new URLSearchParams({endpoint_id:row.endpoint_id,credential_id:accountId})}`} onClick={onNavigate}>查看完整目录与开放模型</Link></section>)}
   <button className="secondary" disabled={catalog.isFetching} onClick={()=>void catalog.refetch()}>重新读取目录状态</button>
  </>:<>
   <p className="muted">以下为当前服务的运行观测，重新读取不会调用上游。</p>
   <PagedReadStatus query={runtime}/>{runtime.data?.observedAt!=null?<p className="muted">运行观测时间：{formatObservedAt(runtime.data.observedAt)}</p>:null}{runtime.data===undefined?null:!runtime.data.items.length?<p>没有此账号的运行观测。请检查接口连接与启用状态；缺失数据不表示免费或额度充足。</p>:runtime.data.items.map(row=><section className="account-evidence-card" key={`${row.provider_id}:${row.channel_id}`}>
    <ResourceIdentity id={row.channel_id} kind="endpoint"/>
    {tab==='quota'?<><EntitlementEvidence entitlement={row.entitlement} expanded/><dl className="fact-grid"><dt>下次额度同步</dt><dd>{row.quota_sync_due_at_ms==null?'未观测':formatObservedAt(row.quota_sync_due_at_ms)}</dd><dt>额度余额</dt><dd>未提供数值观测</dd></dl>{row.runtime_status==='quota_blocked'?<p>当前存在额度阻塞。</p>:null}</>:<dl className="fact-grid"><dt>认证</dt><dd>{authStatusMeta(row.auth_status).label}</dd><dt>调度</dt><dd>{runtimeStatusMeta(row.runtime_status).label}</dd><dt>并发</dt><dd>{row.active_leases} / {row.max_concurrency}</dd><dt>凭据到期</dt><dd>{row.expires_at_ms==null?'未观测':formatObservedAt(row.expires_at_ms)}</dd></dl>}
    <div className="page-actions"><Link to={`/runtime?${new URLSearchParams({account_id:accountId,endpoint_id:row.channel_id,credential_id:accountId})}`} onClick={onNavigate}>查看诊断与恢复</Link><Link to={`/monitoring?${new URLSearchParams({tab:'failures',account_id:accountId,provider_id:row.provider_id,channel_id:row.channel_id})}`} onClick={onNavigate}>失败记录</Link></div>
   </section>)}
   {tab==='quota'?<p className="muted">套餐、额度和模型权限分别判断；未观测的余额保持未知。</p>:null}
   <button className="secondary" disabled={runtime.isFetching} onClick={()=>void runtime.refetch()}>重新读取运行观测</button>
  </>}</div>
 </div>;
}
