import { useInfiniteQuery, useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { useNativeAccounts } from "../accounts/NativeAccounts";
import { accountName, protocolName, nativeConnections } from "../accounts/presentation";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore } from "../config-versions/versionStore";
import { connectModelBatch, type ModelBatchRow } from "../models/connectModelBatch";
import { useModelConnections } from "../models/useModelConnections";
import type { CatalogRow } from "../runtime/model";

type CatalogModelPage=Readonly<{config_version:string;revision:string;current_model_count:number;total_count:number;target:{endpoint_id:string;credential_id:string;snapshot_version:number;observed_at_ms:number;stale_at_ms:number;expires_at_ms:number};items:readonly {model:string;present_in_last_success:boolean}[];next_cursor:string|null}>;

/** A catalog refresh is account-specific, so neither selector may infer its first result. */
export function selectedCatalogEndpoint<T extends {id:string}>(choice:string,options:readonly T[]):T|undefined {
  return options.find((option)=>option.id===choice);
}

/** Directory evidence belongs to one account and requires the operator's explicit choice. */
export function selectedCatalogCredential<T extends {credential_id:string}>(choice:string,options:readonly T[]):T|undefined {
  return options.find((option)=>option.credential_id===choice);
}

export function UpstreamModelBrowser() {
  const navigate=useNavigate();
  const scope=useVersionStore(s=>s.context?.configVersionId);
  const [params]=useSearchParams();
  const topology=useModelConnections();
  const providers=useQuery({queryKey:["upstreams",scope],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const catalog=useQuery({queryKey:["catalog-status",scope],queryFn:()=>call<CatalogRow[]>("getCatalogStatus",{},{versionScoped:true}),enabled:!!scope});
  const credentials=useManagedInventory("credentials");const native=useNativeAccounts();
  const [provider,setProvider]=useState(params.get("upstream_id")??"");
  const [endpointChoice,setEndpoint]=useState(params.get("endpoint_id")??"");
  const [credentialChoice,setCredential]=useState(params.get("credential_id")??"");
  const [search,setSearch]=useState("");const [picked,setPicked]=useState<Set<string>>(new Set());
  const [workingId,setWorkingId]=useState<string>();
  const [batchRows,setBatchRows]=useState<readonly ModelBatchRow[]>([]);
  const endpoints=(topology.data?.endpoints??[]).filter(e=>!provider||e.upstream_id===provider);
  const endpoint=selectedCatalogEndpoint(endpointChoice,endpoints);
  const known=[...(catalog.data??[]).filter(row=>row.endpoint_id===endpoint?.id),
    ...(credentials.data?.pages.flatMap(p=>p.items).filter(c=>c.connections.some(e=>e.id===endpoint?.id)).map(c=>({endpoint_id:endpoint!.id,credential_id:c.credential.id}))??[]),
    ...(native.data?.pages.flatMap(p=>p.items).filter(c=>nativeConnections(c.provider,endpoint?[endpoint]:[]).length).map(c=>({endpoint_id:endpoint!.id,credential_id:c.id}))??[])];
  const targets=[...new Map(known.map(t=>[t.credential_id,t])).values()];
  const target=selectedCatalogCredential(credentialChoice,targets);
  const name=(id:string)=>accountName(credentials.data?.pages.flatMap(p=>p.items).find(c=>c.credential.id===id)?.identity)??accountName(native.data?.pages.flatMap(p=>p.items).find(c=>c.id===id)?.identity)??"未提供账号身份";
  const query=useInfiniteQuery({queryKey:["catalog-models",scope,endpoint?.id,target?.credential_id,search],initialPageParam:undefined as string|undefined,enabled:!!scope&&!!endpoint&&!!target,retry:false,queryFn:({pageParam})=>call<CatalogModelPage>("listCatalogModels",{query:{endpoint_id:endpoint!.id,credential_id:target!.credential_id,limit:100,q:search,...(pageParam?{cursor:pageParam}:{})}},{versionScoped:true}),getNextPageParam:last=>last.next_cursor??undefined});
  const header=query.data?.pages[0]?.target;
  const expired=!!header&&Date.now()>=header.expires_at_ms;
  const rows=query.data?.pages.flatMap(p=>p.items)??[];
  const connected=new Set(topology.data?.candidates.filter(c=>c.endpoint_id===endpoint?.id).map(c=>c.upstream_model)??[]);
  const save=useMutation({mutationFn:async()=>{
    if(!endpoint||!target||!picked.size||picked.size>20||expired)throw new Error("请选择有效目录中的 1–20 个模型。");
    const selected=rows.filter(row=>picked.has(row.model));
    if(selected.length!==picked.size||selected.some(row=>!row.present_in_last_success||row.model.length>256))throw new Error("目录选择已变化，请重新选择。");
    const latest=await call<CatalogModelPage>("listCatalogModels",{query:{endpoint_id:endpoint.id,credential_id:target.credential_id,limit:1}},{versionScoped:true});
    if(latest.target.snapshot_version!==header?.snapshot_version||latest.target.observed_at_ms!==header?.observed_at_ms)throw new Error("目录已更新，请重读后重新选择。");
    const task=await beginConfigurationTask(`从上游目录接入 ${picked.size} 个模型`);setWorkingId(task.version.id);
    await connectModelBatch(task,[...picked],endpoint.id,setBatchRows);
    return task.finish();
  },onSuccess:version=>{setPicked(new Set());useVersionStore.getState().select(version);navigate("/models");}});
  const refresh=useMutation({mutationFn:async()=>{
    if(!endpoint||!target)throw new Error("请先连接账号。");
    return call<{model_count:number}>("refreshCatalogModels",{body:{endpoint_id:endpoint.id,credential_id:target.credential_id}},{versionScoped:true});
  },onSuccess:()=>{setPicked(new Set());void catalog.refetch();void query.refetch();}});
  const resetSelection=()=>{setPicked(new Set());setWorkingId(undefined);setBatchRows([]);save.reset();refresh.reset();};
  return <section className="upstream-model-browser" aria-label="上游模型清单">
    <div className="data-toolbar"><label>提供商<select value={provider} disabled={save.isPending||refresh.isPending} onChange={e=>{setProvider(e.target.value);setEndpoint("");setCredential("");resetSelection();}}><option value="">全部提供商</option>{providers.data?.map(p=><option key={p.id} value={p.id}>{resourceName(p.id,"upstream",p.name)}</option>)}</select></label>
      <label>接口<select value={endpointChoice} disabled={save.isPending||refresh.isPending} onChange={e=>{setEndpoint(e.target.value);setCredential("");resetSelection();}}><option value="">{endpoints.length?"选择接口":"尚未配置接口"}</option>{endpoints.map(e=><option key={e.id} value={e.id}>{resourceName(e.upstream_id,"upstream",providers.data?.find(p=>p.id===e.upstream_id)?.name)} · {protocolName(e.api_format)} · {new URL(e.base_url).host}</option>)}</select></label>
      <label>目录账号<select value={credentialChoice} disabled={save.isPending||refresh.isPending||!endpoint} onChange={e=>{setCredential(e.target.value);resetSelection();}}><option value="">{!endpoint?"请先选择接口":targets.length?"选择目录账号":"尚无目录观测"}</option>{targets.map(t=><option key={t.credential_id} value={t.credential_id}>{name(t.credential_id)}</option>)}</select></label>
    </div>
    <div className="data-toolbar"><input type="search" aria-label="搜索上游模型" placeholder="搜索模型 ID" value={search} disabled={save.isPending||refresh.isPending} onChange={e=>{setSearch(e.target.value);resetSelection();}}/><button disabled={!endpoint||!target||save.isPending||refresh.isPending} onClick={()=>refresh.mutate()}>{refresh.isPending?"正在刷新…":"刷新上游模型"}</button><button className="secondary" disabled={query.isFetching||save.isPending} onClick={()=>{resetSelection();void catalog.refetch();if(target)void query.refetch();}}>重读目录</button><Link to={`/models?add=model${endpoint?`&from_endpoint=${encodeURIComponent(endpoint.id)}`:""}`}>手动批量接入</Link></div>
    {refresh.isError?<p role="alert">{asAppError(refresh.error).message}</p>:null}
    {refresh.isSuccess?<p role="status">上游目录已更新，共 {refresh.data.model_count} 个模型。</p>:null}
    {topology.error||providers.error||catalog.error?<p role="alert">{asAppError(topology.error??providers.error??catalog.error).message}</p>:null}
    {!endpoint?<p className="empty-state">请选择接口，再查看与刷新该接口账号的上游目录。</p>:!target?<p className="empty-state">请选择目录账号；每个账号的目录观测和权限独立保存。</p>:query.isError?<p role="alert" className="empty-state">{asAppError(query.error).status===404?"尚未取得该账号的成功目录。":asAppError(query.error).message}</p>:query.isPending?<p role="status">读取上游模型…</p>:null}
    {header?<p className="stat-sub">观测于 {new Date(header.observed_at_ms).toLocaleString()} · 上游模型 {query.data?.pages[0]?.current_model_count} · 已载入 {rows.length} / {query.data?.pages[0]?.total_count}{query.hasNextPage?"，还有更多":""}{expired?" · 目录已过期，请更新目录后接入":""}</p>:null}
    {rows.length?<><div className="data-toolbar"><span>已选 {picked.size} / 20</span><button disabled={!picked.size||save.isPending||save.isError||query.isError||expired} onClick={()=>save.mutate()}>{save.isPending?"正在接入…":`接入所选模型${picked.size?`（${picked.size}）`:""}`}</button></div><div className="tablewrap"><table><thead><tr><th>选择</th><th>上游模型 ID</th><th>接入状态</th></tr></thead><tbody>{rows.map(row=><tr key={row.model}><td><input type="checkbox" aria-label={`选择 ${row.model}`} checked={picked.has(row.model)} disabled={save.isPending||query.isError||expired||!row.present_in_last_success||row.model.length>256||connected.has(row.model)||(!picked.has(row.model)&&picked.size>=20)} onChange={e=>setPicked(previous=>{const next=new Set(previous);if(e.target.checked)next.add(row.model);else next.delete(row.model);return next;})}/></td><td className="mono">{row.model}</td><td>{connected.has(row.model)?"已连接此接口":row.present_in_last_success?"待接入":"最近目录已不再返回"}</td></tr>)}</tbody></table></div></>:null}
    {!query.isPending&&!query.isError&&target&&header&&!rows.length?<p className="empty-state">{search?"没有匹配的模型。":"上游最近成功返回了空模型目录。"}</p>:null}
    {query.hasNextPage?<button className="secondary" disabled={query.isFetching||query.isError||save.isPending} onClick={()=>void query.fetchNextPage()}>加载更多模型</button>:null}
    {batchRows.length?<section aria-label="模型接入进度" aria-live="polite"><h3>接入结果</h3><ul className="model-batch-results">{batchRows.map(row=><li key={row.model}><code>{row.model}</code><span>{{waiting:"未执行",saving:"正在保存",saved:"已保存，待应用",existing:"连接已存在",uncertain:"未完成，请核对"}[row.state]}</span></li>)}</ul>{save.isError?<p>本批已停止。已保存的修改保留在待应用配置中；请核对后再应用。</p>:null}</section>:null}
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={version=>useVersionStore.getState().select(version)}/>
  </section>;
}
