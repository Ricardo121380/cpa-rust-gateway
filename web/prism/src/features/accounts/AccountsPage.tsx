import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { IdentityDetails } from "../../components/ResourceIdentity";
import { StatusBadge } from "../../components/StatusBadge";
import { beginConfigurationEdit } from "../config-versions/beginEdit";
import { useVersionStore } from "../config-versions/versionStore";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import { OAuthWizard } from "../upstreams/OAuthWizard";
import {useNativeAccounts, type NativeAccount} from "./NativeAccounts";
import {GrokDeviceWizard} from "./GrokDeviceWizard";
import {AccountList, type AccountListRow} from "./AccountList";
import {accountGroups, accountName, accountSource, protocolName} from "./presentation";
import { AddAccountDialog } from "./AddAccountDialog";
import { AccountRuntimePanel } from "./AccountRuntimePanel";
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
  const [status, setStatus] = useState<ManagedCredential>();
  const [notice, setNotice] = useState<string>();
  const refresh = () => Promise.all([client.resetQueries({queryKey: ["managed-inventory"]}),client.resetQueries({queryKey: ["native-accounts"]})]);
  const readIdentity=useMutation({mutationFn:(row:NativeAccount)=>call<{identity:NativeAccount["identity"]}>("refreshNativeAccountIdentity",{path:{account_id:row.id},body:{revision:row.revision}}),onSuccess:(result,row)=>{
    setNativeDetail((current)=>current?.id===row.id?{...current,identity:result.identity}:current);
    setNotice(result.identity.email?"已读取并保存账号身份。":"渠道已返回身份，但没有提供邮箱。");void refresh();void client.resetQueries({queryKey:["accounts"]});
  },onError:(cause)=>{if(asAppError(cause).kind==="conflict")void refresh();}});
  const update = (name: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(name, value); else next.delete(name);
    setParams(next, {replace: true});
  };
  const edit = useMutation({
    mutationFn: (_action: "create" | "edit") => beginConfigurationEdit("账号管理修改"),
    onSuccess: (version, action) => {
      if (action === "create") update("add", "account");
      void client.invalidateQueries({queryKey: ["config-versions"]});
      useVersionStore.getState().select(version);
    },
  });
  const change = useMutation({
    mutationFn: (row: ManagedCredential) => call("updateCredentialStatus", {
      path: {credential_id: row.credential.id},
      body: {status: row.credential.status === "disabled" ? "active" : "disabled", credential_revision: row.credential.revision},
    }, {versionScoped: true, mutating: true}),
    onSuccess: () => {
      setStatus(undefined);
      setNotice("账号状态已保存到待应用配置。");
      void refresh();
      void client.invalidateQueries({queryKey: ["credential"]});
    },
  });
  const error = inventory.isError ? asAppError(inventory.error) : undefined;
  const actions = (row: ManagedCredential) => <div className="page-actions">
              <button className="secondary" onClick={() => {setAuthorizations(undefined);setDetail(row.credential.id);}}>详情</button>
              {row.credential.kind === "oauth_json" ? <button className="secondary" onClick={() => {setAuthorizations(undefined);setOauth(row.credential.id);}}>重新授权</button> : null}
              {context?.status === "draft" ? <button className="secondary" onClick={() => {setAuthorizations(undefined);change.reset(); setStatus(row);}}>{row.credential.status === "disabled" ? "启用" : "停用"}</button> : null}
            </div>;
  const nativeNames={grok_web:"Grok Web",grok_console:"Grok Console",grok_build:"Grok Build"};
  const hasSearch=(values:readonly (string|null|undefined)[])=>values.join(" ").toLocaleLowerCase().includes(search.toLocaleLowerCase());
  const matches=(row:ManagedCredential)=>hasSearch([accountName(row.identity,row.credential.id),row.provider,row.category,row.credential.id,accountSource(row.credential.id)]);
  const matchesNative=(row:NativeAccount)=>hasSearch([accountName(row.identity,row.import_batch_id),"Grok",nativeNames[row.provider],row.id,accountSource(row.import_batch_id)]);
  const ordinaryView=(row:ManagedCredential):AccountListRow=>({
    key:row.credential.id,name:accountName(row.identity,row.credential.id),source:accountSource(row.credential.id),provider:row.provider,
    authentication:row.credential.kind==="oauth_json"?"OAuth 授权":"API / 渠道凭据",
    status:<StatusBadge status={row.credential.status}>{row.credential.status==="active"?"已启用":"已停用"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setConnections(row)}>{row.binding_count===0?"未连接接口":[...new Set(row.connections.map((c)=>protocolName(c.api_format)))].join(" · ")||"查看连接"}<span className="entity-meta">{row.binding_count?`${row.binding_count} 个已配置连接 · 查看`:"添加连接后用于请求"}</span></button>,actions:actions(row),
  });
  const nativeView=(row:NativeAccount):AccountListRow=>({key:row.id,name:accountName(row.identity,row.import_batch_id),source:accountSource(row.import_batch_id),provider:nativeNames[row.provider],authentication:row.provider==="grok_build"?"OAuth 授权":"SSO 授权",
    status:<StatusBadge status={!row.enabled||row.auth_status==="disabled"?"disabled":row.auth_status==="active"?"active":"unauthorized"}>{!row.enabled||row.auth_status==="disabled"?"已停用":row.auth_status==="active"?"已保存授权":"需要重新授权"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setNativeDetail(row)}>渠道账号池<span className="entity-meta">按 {nativeNames[row.provider]} 渠道调度</span></button>,
    actions:<div className="page-actions"><button className="secondary" onClick={()=>setNativeDetail(row)}>详情</button>{!row.identity.email?<button className="secondary" disabled={readIdentity.isPending} onClick={()=>{readIdentity.reset();readIdentity.mutate(row);}}>读取身份</button>:null}{row.provider==="grok_build"?<button className="secondary" onClick={()=>setNativeOauth(row)}>重新授权</button>:null}</div>,
  });
  const groupedView=(group:ManagedCredential[]):AccountListRow=>{
    const first=group[0]!;if(group.length===1)return ordinaryView(first);
    const active=group.filter((row)=>row.credential.status==="active").length;
    const protocols=[...new Set(group.flatMap((row)=>row.connections.map((c)=>protocolName(c.api_format))))];
    return {...ordinaryView(first),source:undefined,authentication:`${group.length} 份授权`,
      status:<StatusBadge status={active?"active":"disabled"}>{active} / {group.length} 份已启用</StatusBadge>,
      connection:<button className="account-connection-link" onClick={()=>setAuthorizations(group)}>{protocols.join(" · ")||"未连接接口"}<span className="entity-meta">查看各份授权的连接</span></button>,
      actions:<button className="secondary" onClick={()=>setAuthorizations(group)}>管理授权</button>};
  };
  return <section className="accounts-page">
    <header className="page-head"><div><h2>账号管理</h2><p>按渠道管理身份、授权与接口连接</p></div>
      <div className="page-actions">
        <button disabled={edit.isPending} onClick={() => edit.mutate("create")}>添加账号</button>
        {context?.status !== "draft" ? <button disabled={edit.isPending} onClick={() => edit.mutate("edit")}>编辑账号配置</button> : null}
        <button className="secondary" onClick={() => void refresh()}>刷新账号</button>
      </div>
    </header>
    {edit.isError ? <p role="alert">{asAppError(edit.error).message}</p> : null}
    {readIdentity.isError?<p role="alert">{asAppError(readIdentity.error).message}</p>:null}
    {notice ? <p role="status">{notice}</p> : null}
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
    {nativeDetail?<Sheet layout="inspector" title="账号详情" onEscape={()=>setNativeDetail(undefined)}>
      <h3>{accountName(nativeDetail.identity,nativeDetail.import_batch_id)??"未提供账号身份"}</h3><p>{nativeNames[nativeDetail.provider]}</p>
      {accountSource(nativeDetail.import_batch_id)?<p>来源：{accountSource(nativeDetail.import_batch_id)}</p>:null}
      <p>账号加入对应 Grok 渠道池，由网关按该渠道选择。它没有普通 API 账号的逐项接口绑定。</p>
      <IdentityDetails entries={[["账号",nativeDetail.id,accountName(nativeDetail.identity)??"未提供账号身份"]]} />
    </Sheet>:null}
    {nativeOauth?<GrokDeviceWizard name={accountName(nativeOauth.identity,nativeOauth.import_batch_id)??"Grok Build 账号"} target={{account_id:nativeOauth.id,revision:nativeOauth.revision}} onClose={()=>{setNativeOauth(undefined);void refresh();}} />:null}
    {["account", "api-key"].includes(params.get("add") ?? "") && context?.status === "draft" ? <AddAccountDialog onClose={() => update("add", "")} onCreated={(notice) => {update("add", ""); setNotice(notice??"账号已保存。"); void refresh();}} /> : null}
    {detail ? <CredentialSheet credentialId={detail} accountName={accountName(rows.find((row)=>row.credential.id===detail)?.identity)} providerName={rows.find((row)=>row.credential.id===detail)?.provider} onClose={() => {setDetail(undefined); void refresh();}} /> : null}
    {oauth ? <OAuthWizard credentialId={oauth} accountName={accountName(rows.find((row)=>row.credential.id===oauth)?.identity)} onClose={() => {setOauth(undefined); void refresh();}} /> : null}
    {status ? <Sheet title={status.credential.status === "disabled" ? "启用账号" : "停用账号"} onEscape={() => !change.isPending && setStatus(undefined)}>
      <p>{accountName(status.identity,status.credential.id)??"未提供账号身份"}</p>
      <p>更改将在配置应用后生效。凭据和历史记录保留。</p>
      {change.isError ? <p role="alert">{asAppError(change.error).message}</p> : null}
      <div className="sheet-actions"><button className="secondary" disabled={change.isPending} onClick={() => setStatus(undefined)}>取消</button><button disabled={change.isPending} onClick={() => change.mutate(status)}>确认更改</button></div>
    </Sheet> : null}
  </section>;
}
