import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { IdentityDetails } from "../../components/ResourceIdentity";
import { StatusBadge } from "../../components/StatusBadge";
import { useVersionStore } from "../config-versions/versionStore";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import { OAuthWizard } from "../upstreams/OAuthWizard";
import {useNativeAccounts, type NativeAccount} from "./NativeAccounts";
import {GrokDeviceWizard} from "./GrokDeviceWizard";
import {AccountList, type AccountListRow} from "./AccountList";
import {accountGroups, accountName, accountSource, protocolName} from "./presentation";
import { AddAccountDialog } from "./AddAccountDialog";
import { NativeAccountDialog } from "./NativeAccountDialog";
import { AccountRuntimePanel } from "./AccountRuntimePanel";
import { AccountBatchDialog, type AccountAction, type AccountTarget } from "./AccountBatchDialog";
import { CredentialUpdateDialog } from "./CredentialUpdateDialog";
import { groupManagedIdentities, useManagedInventory, type ManagedCredential } from "./inventory";

export function AccountsPage() {
  const [params, setParams] = useSearchParams();
  const runtime = params.get("view") === "runtime" || params.has("auth") || params.has("runtime");
  return <>
    <nav className="workspace-navigation" aria-label="账号视图">
      <button aria-pressed={!runtime} onClick={() => setParams(new URLSearchParams())}>全部账号</button>
      <button aria-pressed={runtime} onClick={() => setParams({view: "runtime"})}>运行状态</button>
    </nav>
    {runtime ? <AccountRuntimePanel /> : <ManagedAccounts />}
  </>;
}

function ManagedAccounts() {
  const [params, setParams] = useSearchParams();
  const context = useVersionStore((s) => s.context);
  const client = useQueryClient();
  const search = params.get("q") ?? "";
  const provider = params.get("provider") ?? "";
  const inventory = useManagedInventory("credentials", provider);
  const native = useNativeAccounts();
  const nativeRows = native.data?.pages.flatMap((p)=>p.items) ?? [];
  const selectedCategory = params.get("category") ?? "";
  const [nativeDetail,setNativeDetail] = useState<NativeAccount>();
  const [nativeOauth,setNativeOauth] = useState<NativeAccount>();
  const [connections,setConnections] = useState<ManagedCredential>();
  const [authorizations,setAuthorizations] = useState<ManagedCredential[]>();
  const rows = inventory.data?.pages.flatMap((p) => p.items) ?? [];
  const [detail, setDetail] = useState<string>();
  const [oauth, setOauth] = useState<string>();
  const [more, setMore] = useState<ManagedCredential>();
  const [updating,setUpdating]=useState<ManagedCredential>();
  const [selection, setSelection] = useState<ReadonlySet<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const [batch, setBatch] = useState<{targets:AccountTarget[];action:AccountAction}>();
  const [notice, setNotice] = useState<string>();
  const refresh = () => Promise.all(["managed-inventory", "native-accounts", "accounts", "provider-pools", "runtime-availability", "effective-models"].map((key)=>client.resetQueries({queryKey:[key]})));
  const update = (name: string, value: string) => {
    if (["q", "provider", "category"].includes(name)) setSelection(new Set());
    const next = new URLSearchParams(params);
    if (value) next.set(name, value); else next.delete(name);
    setParams(next, {replace: true});
  };
  const ordinaryTarget=(row:ManagedCredential):AccountTarget=>({id:row.credential.id,native:false,name:accountName(row.identity)??"未提供账号身份",provider:row.provider,enabled:row.credential.status!=="disabled",revision:row.credential.revision});
  const targetKey=(target:AccountTarget)=>`${target.native?"native":"ordinary"}:${target.id}`;
  const toggle=(keys:string[])=>setSelection((current)=>{
    const next=new Set(current);
    if(keys.every((key)=>next.has(key)))keys.forEach((key)=>next.delete(key));
    else if(new Set([...next,...keys]).size<=20)keys.forEach((key)=>next.add(key));
    else setNotice("一次最多选择 20 份授权，请先处理已选账号。");
    return next;
  });
  const startAction=(row:ManagedCredential,action:AccountAction)=>{setMore(undefined);setAuthorizations(undefined);setBatch({targets:[ordinaryTarget(row)],action});};
  const error = inventory.isError ? asAppError(inventory.error) : undefined;
  const actions = (row: ManagedCredential) => <div className="page-actions">
              <button className="secondary" onClick={() => {setAuthorizations(undefined);setDetail(row.credential.id);}}>详情</button>
              {row.credential.kind === "oauth_json" ? <button className="secondary" onClick={() => {setAuthorizations(undefined);setOauth(row.credential.id);}}>重新授权</button> : null}
              <button className="secondary" onClick={() => {setAuthorizations(undefined);setMore(row);}}>更多</button>
            </div>;
  const nativeNames={grok_web:"Grok Web",grok_console:"Grok Console",grok_build:"Grok Build"};
  const nativeTarget=(row:NativeAccount):AccountTarget=>({id:row.id,native:true,name:accountName(row.identity)??"未提供账号身份",provider:nativeNames[row.provider],enabled:row.enabled,revision:row.revision});
  const selectedTargets=[...rows.map(ordinaryTarget),...nativeRows.map(nativeTarget)].filter((target)=>selection.has(targetKey(target)));
  const hasSearch=(values:readonly (string|null|undefined)[])=>values.join(" ").toLocaleLowerCase().includes(search.toLocaleLowerCase());
  const matches=(row:ManagedCredential)=>hasSearch([accountName(row.identity,row.credential.id),row.provider,row.category,row.credential.id,accountSource(row.credential.id)]);
  const matchesNative=(row:NativeAccount)=>hasSearch([accountName(row.identity,row.import_batch_id),"Grok",nativeNames[row.provider],row.id,accountSource(row.import_batch_id)]);
  const ordinaryView=(row:ManagedCredential):AccountListRow=>({
    selected:selection.has(`ordinary:${row.credential.id}`),onSelect:selecting?()=>toggle([`ordinary:${row.credential.id}`]):undefined,
    key:row.credential.id,name:accountName(row.identity,row.credential.id),source:accountSource(row.credential.id),provider:row.provider,
    authentication:row.credential.kind==="oauth_json"?"OAuth 授权":"API / 渠道凭据",
    status:<StatusBadge status={row.credential.status}>{row.credential.status==="active"?"已启用":"已停用"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setConnections(row)}>{row.binding_count===0?"未连接接口":[...new Set(row.connections.map((c)=>protocolName(c.api_format)))].join(" · ")||"查看连接"}<span className="entity-meta">{row.binding_count?`${row.binding_count} 个已配置连接 · 查看`:"添加连接后用于请求"}</span></button>,actions:actions(row),
  });
  const nativeView=(row:NativeAccount):AccountListRow=>({key:row.id,name:accountName(row.identity,row.import_batch_id),source:accountSource(row.import_batch_id),provider:nativeNames[row.provider],authentication:row.provider==="grok_build"?"OAuth 授权":"SSO 授权",
    selected:selection.has(`native:${row.id}`),onSelect:selecting?()=>toggle([`native:${row.id}`]):undefined,
    status:<StatusBadge status={!row.enabled||row.auth_status==="disabled"?"disabled":row.auth_status==="active"?"active":"unauthorized"}>{!row.enabled||row.auth_status==="disabled"?"已停用":row.auth_status==="active"?"已保存授权":"需要重新授权"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setNativeDetail(row)}>渠道账号池<span className="entity-meta">按 {nativeNames[row.provider]} 渠道调度</span></button>,
    actions:<div className="page-actions"><button className="secondary" onClick={()=>setNativeDetail(row)}>详情</button>{row.provider==="grok_build"?<button className="secondary" onClick={()=>setNativeOauth(row)}>重新授权</button>:null}</div>,
  });
  const groupedView=(group:ManagedCredential[]):AccountListRow=>{
    const first=group[0]!;if(group.length===1)return ordinaryView(first);
    const active=group.filter((row)=>row.credential.status==="active").length;
    const protocols=[...new Set(group.flatMap((row)=>row.connections.map((c)=>protocolName(c.api_format))))];
    return {...ordinaryView(first),source:undefined,authentication:`${group.length} 份授权`,
      selected:group.every((row)=>selection.has(`ordinary:${row.credential.id}`)),onSelect:selecting?()=>toggle(group.map((row)=>`ordinary:${row.credential.id}`)):undefined,
      status:<StatusBadge status={active?"active":"disabled"}>{active} / {group.length} 份已启用</StatusBadge>,
      connection:<button className="account-connection-link" onClick={()=>setAuthorizations(group)}>{protocols.join(" · ")||"未连接接口"}<span className="entity-meta">查看各份授权的连接</span></button>,
      actions:<button className="secondary" onClick={()=>setAuthorizations(group)}>管理授权</button>};
  };
  return <section className="accounts-page">
    <header className="page-head"><div><h2>账号管理</h2><p>按渠道管理身份、授权与接口连接</p></div>
      <div className="page-actions">
        <button onClick={() => update("add", "account")}>添加账号</button>
        <button className="secondary" aria-pressed={selecting} onClick={()=>{setSelecting(!selecting);setSelection(new Set());}}>{selecting?"结束选择":"批量管理"}</button>
        <button className="secondary" onClick={() => void refresh()}>刷新账号</button>
      </div>
    </header>
    {notice ? <p role="status">{notice}</p> : null}
    {selecting?<div className="account-batch-toolbar" aria-label="批量账号操作"><span>已选 {selectedTargets.length} / 20 份授权</span><div className="page-actions">{(["enable","disable","remove"] as const).map((action)=><button key={action} className="secondary" disabled={!selectedTargets.length} onClick={()=>setBatch({targets:selectedTargets,action})}>{({enable:"启用",disable:"停用",remove:"移除"})[action]}</button>)}<button className="secondary" disabled={!selection.size} onClick={()=>setSelection(new Set())}>清除选择</button></div></div>:null}
    <div className="account-directory-toolbar">
      <label className="account-search"><span className="sr-only">搜索账号</span><input aria-label="搜索账号" placeholder="搜索已加载的邮箱、用户名或渠道" value={search} onChange={(e)=>update("q",e.target.value)} /></label>
      <span className="entity-meta">已加载 {rows.length + (provider ? 0 : nativeRows.length)} 份授权</span>
      {provider?<button className="secondary" onClick={()=>update("provider","")}>清除提供商筛选</button>:null}
    </div>
    <nav className="account-category-filter" aria-label="账号类别">
      <button aria-pressed={!selectedCategory} onClick={()=>update("category","")}>全部</button>
      {accountGroups.map((group)=><button key={group.id} aria-pressed={selectedCategory===group.id} onClick={()=>update("category",group.id)}>{group.name}</button>)}
    </nav>
    {error?<div role="alert" className="empty-state">{error.message}<button onClick={()=>void refresh()}>重新读取账号</button></div>:null}
    {native.isError&&!provider?<div role="alert" className="empty-state">Grok：{asAppError(native.error).message}<button onClick={()=>void client.resetQueries({queryKey:["native-accounts"]})}>重新读取 Grok</button></div>:null}
    <div className="account-directory">
      {accountGroups.filter((group)=>!selectedCategory||selectedCategory===group.id).map((group)=>{
        const ordinary=rows.filter((row)=>(row.category??"api")===group.id&&matches(row));
        const identities=groupManagedIdentities(ordinary);
        const grok=provider?[]:nativeRows.filter(matchesNative);
        const loading=group.id==="grok"?native.isPending:!!context&&inventory.isPending;
        const failed=group.id==="grok"?native.isError:inventory.isError;
        return <section className="account-group" key={group.id} aria-label={`${group.name} 账号`}>
          <header className="account-directory-head"><div><h3>{group.name}</h3><span className="entity-meta">{group.description}</span></div><span className="account-group-count">{loading?"读取中":failed?"读取失败":`${group.id==="grok"?grok.length:identities.length}`}</span></header>
          {loading?<p className="account-group-empty">读取账号…</p>:failed?<p className="account-group-empty">暂时无法显示此类别</p>:group.id==="grok"?
            (["grok_web","grok_console","grok_build"] as const).map((channel)=><section className="account-subgroup" key={channel} aria-label={nativeNames[channel]}><h4>{nativeNames[channel]}</h4><AccountList rows={grok.filter((row)=>row.provider===channel).map(nativeView)} /></section>):
            <AccountList rows={identities.map(groupedView)} />}
        </section>;
      })}
    </div>
    <div className="data-footer account-directory-footer"><span>搜索和分类结果基于已加载账号{inventory.hasNextPage||native.hasNextPage?"，可继续加载更多":""}</span><div className="page-actions">
      {inventory.hasNextPage?<button className="secondary" disabled={inventory.isFetchingNextPage||inventory.isError} onClick={()=>void inventory.fetchNextPage()}>加载更多账号</button>:null}
      {!provider&&native.hasNextPage?<button className="secondary" disabled={native.isFetchingNextPage||native.isError} onClick={()=>void native.fetchNextPage()}>加载更多 Grok 账号</button>:null}
    </div></div>
    {authorizations?<Sheet layout="inspector" title="关联授权" onEscape={()=>setAuthorizations(undefined)}>
      <h3>{accountName(authorizations[0]?.identity)}</h3><p>该身份有 {authorizations.length} 份授权，分别保留连接和状态。操作只影响所选授权。</p>
      <div className="account-grants">{authorizations.map((row)=><section key={row.credential.id} className="account-grant" aria-label="授权记录">
        <div className="page-actions"><strong>{row.provider} · {row.credential.kind==="oauth_json"?"OAuth 授权":"渠道凭据"}</strong>{ordinaryView(row).status}</div>
        <p>{row.connections.length?row.connections.map((c)=>`${protocolName(c.api_format)}${c.host?` · ${c.host}`:""}`).join(" / "):"未连接接口"}</p>
        {accountSource(row.credential.id)?<p className="entity-meta">来源 · {accountSource(row.credential.id)}</p>:null}
        <IdentityDetails entries={[["账号",row.credential.id,accountName(row.identity)],["提供商",row.credential.upstream_id,row.provider]]}/>
        {actions(row)}
      </section>)}</div>
    </Sheet>:null}
    {connections?<Sheet layout="inspector" title="接口连接" onEscape={()=>setConnections(undefined)}>
      <p>{accountName(connections.identity,connections.credential.id)??"未提供账号身份"}</p>
      <p className="muted">网关通过这些接口使用该账号。不同接口可以支持不同请求格式；配置已启用不代表当前正在使用。</p>
      {connections.binding_count===0?<p>尚未连接到接口，暂不用于请求。</p>:<ul className="account-connections">{connections.connections.map((connection)=><li key={connection.id}><strong>{protocolName(connection.api_format)}</strong><span>{connection.host??"未提供主机信息"}</span><StatusBadge status={connection.enabled?"active":"disabled"}>{connection.enabled?"配置已启用":"配置已停用"}</StatusBadge></li>)}</ul>}
      {connections.binding_count>connections.connections.length?<p>仅预览前 {connections.connections.length} 个连接，共 {connections.binding_count} 个。</p>:null}
      <Link to={`/upstreams?upstream_id=${encodeURIComponent(connections.credential.upstream_id)}`} onClick={()=>setConnections(undefined)}>管理接口连接</Link>
    </Sheet>:null}
    {nativeDetail?<NativeAccountDialog account={nativeDetail} onClose={()=>setNativeDetail(undefined)} onAuthorize={()=>{setNativeOauth(nativeDetail);setNativeDetail(undefined);}} onChanged={(notice)=>{setNativeDetail(undefined);setNotice(notice);void refresh();void client.resetQueries({queryKey:["accounts"]});}}/>:null}
    {nativeOauth?<GrokDeviceWizard name={accountName(nativeOauth.identity,nativeOauth.import_batch_id)??"Grok Build 账号"} target={{account_id:nativeOauth.id,revision:nativeOauth.revision}} onClose={()=>{setNativeOauth(undefined);void refresh();}} />:null}
    {["account", "api-key"].includes(params.get("add") ?? "") ? <AddAccountDialog onClose={() => update("add", "")} onCreated={(notice) => {update("add", ""); setNotice(notice??"账号已保存。"); void refresh();}} /> : null}
    {detail ? <CredentialSheet credentialId={detail} accountName={accountName(rows.find((row)=>row.credential.id===detail)?.identity)} providerName={rows.find((row)=>row.credential.id===detail)?.provider} onClose={() => {setDetail(undefined); void refresh();}} /> : null}
    {oauth ? <OAuthWizard credentialId={oauth} accountName={accountName(rows.find((row)=>row.credential.id===oauth)?.identity)} onClose={() => {setOauth(undefined); void refresh();}} /> : null}
    {more?<Sheet title="账号操作" onEscape={()=>setMore(undefined)}><h3>{accountName(more.identity)??more.provider}</h3><div className="sheet-actions"><button onClick={()=>{setUpdating(more);setMore(undefined);}}>更新凭据</button><button className="secondary" onClick={()=>startAction(more,more.credential.status==="disabled"?"enable":"disable")}>{more.credential.status==="disabled"?"启用":"停用"}账号</button><button className="danger" onClick={()=>startAction(more,"remove")}>移除授权</button></div></Sheet>:null}
    {updating?<CredentialUpdateDialog account={updating} onClose={()=>setUpdating(undefined)} onSaved={(version)=>{setUpdating(undefined);void refresh();useVersionStore.getState().select(version);}}/>:null}
    {batch?<AccountBatchDialog targets={batch.targets} action={batch.action} onClose={()=>setBatch(undefined)} onCompleted={(message,version)=>{setBatch(undefined);setSelection(new Set());setNotice(message);void refresh();if(version)useVersionStore.getState().select(version);}}/>:null}
  </section>;
}
