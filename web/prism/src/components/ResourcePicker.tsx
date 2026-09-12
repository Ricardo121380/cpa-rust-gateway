import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../api/client";
import { asAppError } from "../api/errors";
import { useVersionStore } from "../features/config-versions/versionStore";
import { accountName, protocolName } from "../features/accounts/presentation";
import type { ManagedCredential, ManagedEndpoint } from "../features/accounts/inventory";
import type { PoolAccount } from "../features/runtime/model";
import { resourceName } from "../utils/resourceNames";

type Kind="account"|"endpoint"|"upstream"|"route"|"group"|"key";
type Item=ManagedCredential|ManagedEndpoint|PoolAccount|{id:string;name?:string;prefix?:string;public_model_id?:string};
const operations={account:"listManagedCredentials",endpoint:"listManagedEndpoints",upstream:"listUpstreams",route:"listRoutes",group:"listAccessGroups",key:"listClientKeys"} as const;
export const resourceFilterKinds:Readonly<Record<string,Kind|undefined>>={provider_id:"upstream",upstream_id:"upstream",channel_id:"endpoint",endpoint_id:"endpoint",credential_id:"account",account_id:"account",route_id:"route",access_group_id:"group",client_key_id:"key"};
/** Select human-readable resources; only the original ID is submitted to the gateway. */
export function ResourcePicker({kind,name,value,defaultValue="",onChange,required=false,disabled=false,runtime=false,allowCustom=false}:Readonly<{
  kind:Kind;name?:string;value?:string;defaultValue?:string;onChange?:(value:string)=>void;required?:boolean;disabled?:boolean;runtime?:boolean;allowCustom?:boolean;
}>) {
  const scope=useVersionStore(s=>s.context?.configVersionId);
  const [local,setLocal]=useState(defaultValue);
  const [manual,setManual]=useState(false);
  const [reference,setReference]=useState("");
  const selected=value??local;
  const select=(next:string)=>{setLocal(next);onChange?.(next);};
  const useReference=()=>{const next=reference.trim();if(next){select(next);setReference("");setManual(false);}};
  const query=useInfiniteQuery({
    queryKey:["resource-picker",scope,kind,runtime],initialPageParam:undefined as string|undefined,
    enabled:scope!==undefined,retry:false,
    queryFn:({pageParam})=>call<{items:Item[];next_cursor?:string|null}|Item[]>(runtime?"listProviderAccountPools":operations[kind],{query:["upstream","group","key"].includes(kind)?{}:{limit:100,...(pageParam?{cursor:pageParam}:{})}},{versionScoped:!runtime}),
    getNextPageParam:page=>Array.isArray(page)?undefined:page.next_cursor??undefined,
  });
  const options=(query.data?.pages.flatMap(page=>Array.isArray(page)?page:page.items)??[]).map(row=>{
    if("account_id" in row)return {id:row.account_id,label:`${accountName(row.presentation?.identity)??"未提供账号身份"} · ${row.presentation?.provider??resourceName(row.provider_id,"upstream")}`};
    if("credential" in row)return {id:row.credential.id,label:`${accountName(row.identity)??"未提供账号身份"} · ${row.provider}`};
    if("base_url" in row){let host="";try{host=new URL(row.base_url).host;}catch{/* Invalid persisted URL is shown without guessing a host. */}
      return {id:row.id,label:`${resourceName(row.id,"endpoint")} · ${protocolName(row.api_format)}${host?` · ${host}`:""}`};}
    return {id:row.id,label:kind==="key"&&row.prefix?row.prefix:resourceName(row.id,kind,row.name)};
  });
  const ids=new Set([selected,...options.map(row=>row.id)]);
  let historyOption="__history_resource";
  while(ids.has(historyOption))historyOption+="_";
  return <>
    <select name={name} value={selected} required={required} disabled={disabled} onChange={e=>{if(allowCustom&&e.currentTarget.selectedOptions[0]?.dataset.historyOption)setManual(true);else select(e.target.value);}}>
      <option value="">{required?"选择资源":"全部 / 未指定"}</option>
      {selected&&!options.some(row=>row.id===selected)?<option value={selected}>{resourceName(selected,kind)} · 当前引用</option>:null}
      {[...new Map(options.map(row=>[row.id,row])).values()].map(row=><option key={row.id} value={row.id}>{row.label}</option>)}
      {allowCustom?<option value={historyOption} data-history-option="true">指定历史资源…</option>:null}
    </select>
    {manual?<span><input aria-label="历史资源引用" placeholder="粘贴历史资源引用" maxLength={128} value={reference} onChange={e=>setReference(e.target.value)} onKeyDown={e=>{if(e.key==="Enter"){e.preventDefault();useReference();}}}/><button type="button" className="secondary" onClick={useReference}>使用此引用</button><button type="button" className="secondary" onClick={()=>{setManual(false);setReference("");}}>取消</button></span>:null}
    {query.isError?<span role="alert">{asAppError(query.error).message}<button type="button" className="secondary" onClick={()=>void query.refetch()}>重新读取</button></span>:null}
    {query.hasNextPage?<button type="button" className="secondary" disabled={query.isFetchingNextPage} onClick={()=>void query.fetchNextPage()}>加载更多资源</button>:null}
  </>;
}
