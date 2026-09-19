import { CancelledError, useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { useNativeAccounts } from "../accounts/NativeAccounts";
import { accountName, protocolName, nativeConnections } from "../accounts/presentation";
import { useVersionStore } from "../config-versions/versionStore";
import { useSessionStore } from "../../session/sessionStore";
import { useModelConnections } from "../models/useModelConnections";
import type { CatalogRow } from "../runtime/model";
import { CatalogConnectDialog, type CatalogConnectionSelection } from "./CatalogConnectDialog";

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
  const queryClient=useQueryClient();
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
  const [connection,setConnection]=useState<CatalogConnectionSelection>();
  const [selectionError,setSelectionError]=useState<string>();
  const currentChoice=useRef({scope,endpointChoice,credentialChoice});
  currentChoice.current={scope,endpointChoice,credentialChoice};
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
  const confirmSelection=()=>{
    const first=query.data?.pages[0];
    if(!endpoint||!target||!header||!first||!picked.size||picked.size>20||expired||query.isError||query.isFetchNextPageError){setSelectionError("请选择有效目录中的 1–20 个模型。");return;}
    if(!query.data?.pages.every(page=>page.config_version===first.config_version&&page.revision===first.revision&&page.target.endpoint_id===endpoint.id&&page.target.credential_id===target.credential_id&&page.target.snapshot_version===header.snapshot_version&&page.target.observed_at_ms===header.observed_at_ms)){setSelectionError("目录分页证据已变化，请重新读取后选择。");return;}
    const selected=rows.filter(row=>picked.has(row.model));
    if(selected.length!==picked.size||selected.some(row=>!row.present_in_last_success||row.model.length>256)){setSelectionError("目录选择已变化，请重新选择。");return;}
    setSelectionError(undefined);
    setConnection({configVersion:first.config_version,configRevision:first.revision,endpointId:endpoint.id,endpoint:{...endpoint},credentialId:target.credential_id,accountName:name(target.credential_id),snapshotVersion:header.snapshot_version,observedAtMs:header.observed_at_ms,expiresAtMs:header.expires_at_ms,models:selected.map(row=>row.model)});
  };
  const refresh=useMutation({mutationFn:async(input:{endpointId:string;credentialId:string;configVersion:string})=>{
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const value=await call<{model_count:number}>("refreshCatalogModels",{body:{endpoint_id:input.endpointId,credential_id:input.credentialId},headers:{"X-Config-Version":input.configVersion}});
    if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});
    return {...value,...input};
  },onSuccess:(_,input)=>{
    void queryClient.invalidateQueries({queryKey:["catalog-status",input.configVersion]});
    void queryClient.invalidateQueries({queryKey:["catalog-models",input.configVersion,input.endpointId,input.credentialId]});
    if(currentChoice.current.scope===input.configVersion&&currentChoice.current.endpointChoice===input.endpointId&&currentChoice.current.credentialChoice===input.credentialId)setPicked(new Set());
  }});
  const resetSelection=()=>{setPicked(new Set());setSelectionError(undefined);refresh.reset();};
  return <section className="upstream-model-browser" aria-label="上游模型清单">
    <div className="data-toolbar"><label>提供商<select value={provider} disabled={!!connection||refresh.isPending} onChange={e=>{setProvider(e.target.value);setEndpoint("");setCredential("");resetSelection();}}><option value="">全部提供商</option>{providers.data?.map(p=><option key={p.id} value={p.id}>{resourceName(p.id,"upstream",p.name)}</option>)}</select></label>
      <label>接口<select value={endpointChoice} disabled={!!connection||refresh.isPending} onChange={e=>{setEndpoint(e.target.value);setCredential("");resetSelection();}}><option value="">{endpoints.length?"选择接口":"尚未配置接口"}</option>{endpoints.map(e=><option key={e.id} value={e.id}>{resourceName(e.upstream_id,"upstream",providers.data?.find(p=>p.id===e.upstream_id)?.name)} · {protocolName(e.api_format)} · {new URL(e.base_url).host}</option>)}</select></label>
      <label>目录账号<select value={credentialChoice} disabled={!!connection||refresh.isPending||!endpoint} onChange={e=>{setCredential(e.target.value);resetSelection();}}><option value="">{!endpoint?"请先选择接口":targets.length?"选择目录账号":"尚无目录观测"}</option>{targets.map(t=><option key={t.credential_id} value={t.credential_id}>{name(t.credential_id)}</option>)}</select></label>
    </div>
    <div className="data-toolbar"><input type="search" aria-label="搜索上游模型" placeholder="搜索模型 ID" value={search} disabled={!!connection||refresh.isPending} onChange={e=>{setSearch(e.target.value);resetSelection();}}/><button disabled={!endpoint||!target||!!connection||refresh.isPending} onClick={()=>endpoint&&target&&scope&&refresh.mutate({endpointId:endpoint.id,credentialId:target.credential_id,configVersion:scope})}>{refresh.isPending?"正在刷新…":"刷新上游模型"}</button><button className="secondary" disabled={query.isFetching||!!connection||refresh.isPending} onClick={()=>{resetSelection();void catalog.refetch();if(target)void query.refetch();}}>重读目录</button>{refresh.isPending||connection?<span aria-disabled="true">手动批量接入</span>:<Link to={`/models?add=model${endpoint?`&from_endpoint=${encodeURIComponent(endpoint.id)}`:""}`}>手动批量接入</Link>}</div>
    {refresh.isError&&refresh.variables?.configVersion===scope&&refresh.variables.endpointId===endpointChoice&&refresh.variables.credentialId===credentialChoice?<p role="alert">{asAppError(refresh.error).message}。结果未确认时请重读目录，不要重复刷新请求。</p>:null}
    {refresh.isSuccess&&refresh.data.configVersion===scope&&refresh.data.endpointId===endpointChoice&&refresh.data.credentialId===credentialChoice?<p role="status">此账号的上游目录已更新，共 {refresh.data.model_count} 个模型。</p>:null}
    {topology.error||providers.error||catalog.error?<p role="alert">{asAppError(topology.error??providers.error??catalog.error).message}</p>:null}
    {!endpoint?<p className="empty-state">请选择接口，再查看与刷新该接口账号的上游目录。</p>:!target?<p className="empty-state">请选择目录账号；每个账号的目录观测和权限独立保存。</p>:query.isError&&!query.isFetchNextPageError?<p role="alert" className="empty-state">{asAppError(query.error).status===404?"尚未取得该账号的成功目录。":asAppError(query.error).message}</p>:query.isPending?<p role="status">读取上游模型…</p>:null}
    {header?<p className="stat-sub">观测于 {new Date(header.observed_at_ms).toLocaleString()} · 上游模型 {query.data?.pages[0]?.current_model_count} · 已载入 {rows.length} / {query.data?.pages[0]?.total_count}{query.hasNextPage?"，还有更多":""}{expired?" · 目录已过期，请更新目录后接入":""}</p>:null}
    {rows.length?<><div className="data-toolbar"><span>已选 {picked.size} / 20</span><button disabled={!picked.size||!!connection||query.isError||query.isFetchNextPageError||expired} onClick={confirmSelection}>{`接入所选模型${picked.size?`（${picked.size}）`:""}`}</button></div><div className="tablewrap"><table><thead><tr><th>选择</th><th>上游模型 ID</th><th>接入状态</th></tr></thead><tbody>{rows.map(row=><tr key={row.model}><td><input type="checkbox" aria-label={`选择 ${row.model}`} checked={picked.has(row.model)} disabled={!!connection||query.isError||query.isFetchNextPageError||expired||!row.present_in_last_success||row.model.length>256||connected.has(row.model)||(!picked.has(row.model)&&picked.size>=20)} onChange={e=>setPicked(previous=>{const next=new Set(previous);if(e.target.checked)next.add(row.model);else next.delete(row.model);return next;})}/></td><td className="mono">{row.model}</td><td>{connected.has(row.model)?"已连接此接口":row.present_in_last_success?"待接入":"最近目录已不再返回"}</td></tr>)}</tbody></table></div></>:null}
    {query.isFetchNextPageError?<p role="alert">后续目录页读取失败：{asAppError(query.error).message}。请重新读取目录，再核对选择。</p>:null}
    {query.hasNextPage?<button className="secondary" disabled={query.isFetchingNextPage||query.isFetchNextPageError||!!connection||refresh.isPending||expired} onClick={()=>void query.fetchNextPage()}>{query.isFetchingNextPage?"正在加载…":"加载更多模型"}</button>:null}
    {!query.isPending&&!query.isError&&target&&header&&!rows.length?<p className="empty-state">{search?"没有匹配的模型。":"上游最近成功返回了空模型目录。"}</p>:null}
    {selectionError?<p role="alert">{selectionError}</p>:null}
    {connection?<CatalogConnectDialog selection={connection} onClose={()=>setConnection(undefined)} onSaved={version=>{setConnection(undefined);setPicked(new Set());useVersionStore.getState().select(version);navigate("/models");}}/>:null}
  </section>;
}
