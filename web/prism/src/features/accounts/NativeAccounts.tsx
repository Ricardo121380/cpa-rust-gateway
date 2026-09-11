import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { GrokDeviceWizard } from "./GrokDeviceWizard";
type Account=Readonly<{id:string;provider:string;auth_status:string;enabled:boolean;revision:number;import_batch_id:string}>;
type Page=Readonly<{items:readonly Account[];next_cursor:string|null}>;
export function NativeAccounts({search}:Readonly<{search:string}>) {
  const client=useQueryClient();
  const [target,setTarget]=useState<Account>();
  const query=useInfiniteQuery({queryKey:["native-accounts",search],initialPageParam:undefined as string|undefined,retry:false,
    queryFn:({pageParam})=>call<Page>("listNativeAccounts",{query:{limit:100,...(search?{q:search}:{}),...(pageParam?{cursor:pageParam}:{})}}),getNextPageParam:(last)=>last.next_cursor??undefined});
  const rows=query.data?.pages.flatMap((p)=>p.items)??[];
  const names:Record<string,string>={grok_build:"Grok Build",grok_console:"Grok Console",grok_web:"Grok Web"};
  return <section className="native-accounts"><h3>Grok Build / Console / Web</h3>
    {query.isError?<p role="alert">{asAppError(query.error).message}<button className="secondary" onClick={()=>void client.resetQueries({queryKey:["native-accounts",search],exact:true})}>重新读取</button></p>:query.isPending?<p>读取 Grok 账号…</p>:rows.length===0?<p className="muted">{search?"没有匹配的 Grok 账号":"尚未添加 Grok 账号"}</p>:
      <div className="native-account-grid">{rows.map((row)=><article className="data-panel" key={row.id}>
        <strong><ResourceIdentity id={row.id} kind="account" name={row.import_batch_id}/></strong>
        <p>{names[row.provider]} · {row.auth_status==="active"?"已保存授权":row.auth_status==="reauth_required"?"需要重新授权":"已停用"}</p>
        {!row.enabled?<p>已禁用</p>:null}
        {row.provider==="grok_build"?<button className="secondary" onClick={()=>setTarget(row)}>重新授权</button>:null}
      </article>)}</div>}
    {query.hasNextPage?<button disabled={query.isFetchingNextPage||query.isError} onClick={()=>void query.fetchNextPage()}>加载更多 Grok 账号</button>:null}
    {target?<GrokDeviceWizard name={target.import_batch_id} target={{account_id:target.id,revision:target.revision}} onClose={()=>setTarget(undefined)}/>:null}
  </section>;
}
