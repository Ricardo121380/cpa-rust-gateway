import { AccountRuntimeSummary, useAccountRuntimeSummary } from "./AccountRuntimeSummary";
import { KiroDeviceDialog } from "./KiroDeviceDialog";
import { KimiDeviceDialog } from "./KimiDeviceDialog";
import {useAccountDirectory} from "./useAccountDirectory";
import { useModelConnections } from "../models/useModelConnections";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { IdentityDetails } from "../../components/ResourceIdentity";
import { StatusBadge } from "../../components/StatusBadge";
import { useVersionStore } from "../config-versions/versionStore";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import { AuthorizationCodeDialog } from "./AuthorizationCodeDialog";
import { OAuthWizard } from "../upstreams/OAuthWizard";
import {type NativeAccount} from "./NativeAccounts";
import {GrokDeviceWizard} from "./GrokDeviceWizard";
import {AccountList, type AccountListRow} from "./AccountList";
import {accountGroups, accountName, accountSource, protocolName, nativeConnections} from "./presentation";
import { AddAccountDialog } from "./AddAccountDialog";
import { NativeAccountDialog } from "./NativeAccountDialog";
import { AccountRuntimePanel } from "./AccountRuntimePanel";
import { AccountBatchDialog, type AccountAction, type AccountTarget } from "./AccountBatchDialog";
import { CredentialUpdateDialog } from "./CredentialUpdateDialog";
import { groupManagedIdentities, type ManagedCredential } from "./inventory";
import { accountActionTargetKey, freezeAccountActionTargets } from "./accountActionModel";

export function AccountsPage() {
  const [params, setParams] = useSearchParams();
  const runtime = params.get("view") === "runtime" || params.has("auth") || params.has("runtime");
  const navigation = <nav className="workspace-navigation" aria-label="账号视图">
      <button aria-pressed={!runtime} onClick={() => { if (runtime) setParams(new URLSearchParams()); }}>全部账号</button>
      <button aria-pressed={runtime} onClick={() => { if (!runtime) setParams({view: "runtime"}); }}>运行状态</button>
    </nav>;
  return runtime ? <>{navigation}<AccountRuntimePanel /></> : <ManagedAccounts navigation={navigation} />;
}

function ManagedAccounts({navigation}: Readonly<{navigation: ReactNode}>) {
  const [params, setParams] = useSearchParams();
  const context = useVersionStore((s) => s.context);
  const client = useQueryClient();
  const search = params.get("q") ?? "";
  const provider = params.get("provider") ?? "";
  const [searchInput,setSearchInput]=useState(search);
  useEffect(()=>setSearchInput(search),[search]);
  const selectedCategory = params.get("category") ?? "";
  const selectedStatus=params.get("status")??"";
  const sort=params.get("sort")??"name";
  const plan=params.get("plan")??"";const withoutPlan=params.get("without_plan")==="true";
  const inventory=useAccountDirectory({q:search,category:selectedCategory,status:selectedStatus,sort,upstream_id:provider,plan,without_plan:withoutPlan?"true":""});
  const native=inventory;
  const runtimeSnapshot=useAccountRuntimeSummary();
  const topology = useModelConnections();
  const directory=inventory.data?.pages.flatMap(p=>p.items)??[];
  const nativeRows=directory.flatMap(row=>row.native_account?[row.native_account]:[]);
  const [nativeDetail,setNativeDetail] = useState<NativeAccount>();
  const [nativeOauth,setNativeOauth] = useState<NativeAccount>();
  const [connections,setConnections] = useState<ManagedCredential>();
  const [authorizations,setAuthorizations] = useState<ManagedCredential[]>();
  const rows=directory.flatMap(row=>row.managed?[row.managed]:[]);
  const [detail, setDetail] = useState<string>();
  const [oauth, setOauth] = useState<string>();
  const [claudeOauth,setClaudeOauth] = useState<ManagedCredential>();
  const [kiroOauth,setKiroOauth] = useState<ManagedCredential>();
  const [kimiOauth,setKimiOauth] = useState<ManagedCredential>();
  const [more, setMore] = useState<ManagedCredential>();
  const [updating,setUpdating]=useState<ManagedCredential>();
  const [selection, setSelection] = useState<ReadonlySet<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const [batch, setBatch] = useState<{targets:readonly AccountTarget[];action:AccountAction}>();
  const [notice, setNotice] = useState<string>();
  // A delayed search replace is background work, not an intent to leave a
  // newly opened account operation. Retire its timer until that owner closes.
  const operationOpen = ["account", "api-key"].includes(params.get("add") ?? "") ||
    !!(nativeDetail || nativeOauth || connections || authorizations || detail || oauth ||
      claudeOauth || kiroOauth || kimiOauth || more || updating || batch);
  useEffect(() => {
    if (operationOpen || searchInput === search) return;
    const timer = setTimeout(() => {
      setSelection(new Set());
      const next = new URLSearchParams(params);
      if (searchInput) next.set("q", searchInput); else next.delete("q");
      setParams(next, { replace: true });
    }, 300);
    return () => clearTimeout(timer);
  }, [operationOpen, searchInput, search, params, setParams]);
  const refresh = () => Promise.all(["account-list-runtime", "account-directory", "managed-inventory", "native-accounts", "accounts", "provider-pools", "runtime-availability", "effective-models"].map((key)=>client.invalidateQueries({queryKey:[key]})));
  const update = (name: string, value: string) => {
    if (["q", "provider", "category", "status", "sort"].includes(name)) setSelection(new Set());
    const next = new URLSearchParams(params);
    if (value) next.set(name, value); else next.delete(name);
    setParams(next, {replace: true});
  };
  const ordinaryStatus=(row:ManagedCredential)=>directory.find(item=>!item.native&&item.id===row.credential.id)?.status??"disabled";
  const ordinaryTarget=(row:ManagedCredential):AccountTarget=>({id:row.credential.id,native:false,upstreamId:row.credential.upstream_id,name:accountName(row.identity)??"未提供账号身份",provider:row.provider,enabled:ordinaryStatus(row)!=="disabled",revision:row.credential.revision});
  const targetKey=accountActionTargetKey;
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
              {directory.find(item=>item.id===row.credential.id&&!item.native)?.operations.includes("reauthorize") ? <button className="secondary" onClick={() => {setAuthorizations(undefined);if(row.category==="claude")setClaudeOauth(row);else if(row.category==="kiro")setKiroOauth(row);else if(row.category==="kimi")setKimiOauth(row);else setOauth(row.credential.id);}}>重新授权</button> : null}
              <button className="secondary" onClick={() => {setAuthorizations(undefined);setMore(row);}}>更多</button>
            </div>;
  const nativeNames={grok_web:"Grok Web",grok_console:"Grok Console",grok_build:"Grok Build"};
  const nativeTarget=(row:NativeAccount):AccountTarget=>({id:row.id,native:true,nativeProvider:row.provider,name:accountName(row.identity)??"未提供账号身份",provider:nativeNames[row.provider],enabled:row.enabled,revision:row.revision});
  const selectedTargets=freezeAccountActionTargets([...rows.map(ordinaryTarget),...nativeRows.map(nativeTarget)].filter((target)=>selection.has(targetKey(target))));
  const ordinaryView=(row:ManagedCredential):AccountListRow=>({
    selected:selection.has(`ordinary:${row.credential.id}`),onSelect:selecting?()=>toggle([`ordinary:${row.credential.id}`]):undefined,
    key:row.credential.id,name:accountName(row.identity,row.credential.id),source:accountSource(row.credential.id),provider:row.provider,
    authentication:row.authentication==="oauth"||row.credential.kind==="oauth_json"?"OAuth 授权":row.authentication==="api_key"?"API Key":"渠道凭据",
    plan:row.plan,planSource:row.plan_source,
    runtime:<AccountRuntimeSummary snapshot={runtimeSnapshot} ids={[row.credential.id]} providerId={row.credential.upstream_id}/>,
    status:<StatusBadge status={ordinaryStatus(row)==="enabled"?"active":ordinaryStatus(row)==="reauth_required"?"unauthorized":"disabled"}>{ordinaryStatus(row)==="enabled"?"已启用":ordinaryStatus(row)==="reauth_required"?"需要重新授权":"已停用"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setConnections(row)}>{row.binding_count===0?"未连接接口":[...new Set(row.connections.map((c)=>protocolName(c.api_format)))].join(" · ")||"查看连接"}<span className="entity-meta">{row.binding_count?`${row.binding_count} 个已配置连接 · 查看`:"添加连接后用于请求"}</span></button>,actions:actions(row),
  });
  const nativeView=(row:NativeAccount):AccountListRow=>({key:row.id,name:accountName(row.identity,row.import_batch_id),source:accountSource(row.import_batch_id),provider:nativeNames[row.provider],authentication:row.provider==="grok_build"?"OAuth 授权":"SSO 授权",
    runtime:<AccountRuntimeSummary snapshot={runtimeSnapshot} ids={[row.id]} nativeKind={{grok_build:"grok_build_oauth",grok_web:"grok_web_sso",grok_console:"grok_console_sso"}[row.provider]}/>,
    plan:directory.find(item=>item.native&&item.id===row.id)?.plan,planSource:directory.find(item=>item.native&&item.id===row.id)?.plan_source,
    selected:selection.has(`native:${row.id}`),onSelect:selecting?()=>toggle([`native:${row.id}`]):undefined,
    status:<StatusBadge status={!row.enabled||row.auth_status==="disabled"?"disabled":row.auth_status==="active"?"active":"unauthorized"}>{!row.enabled||row.auth_status==="disabled"?"已停用":row.auth_status==="active"?"已保存授权":"需要重新授权"}</StatusBadge>,
    connection:<button className="account-connection-link" onClick={()=>setNativeDetail(row)}>{topology.isError?"接口读取失败":!topology.data?"读取接口…":[...new Set(nativeConnections(row.provider,topology.data.endpoints).map(c=>protocolName(c.api_format)))].join(" · ")||"未配置接口"}<span className="entity-meta">查看接口连接</span></button>,
    actions:<div className="page-actions"><button className="secondary" onClick={()=>setNativeDetail(row)}>详情</button>{row.provider==="grok_build"?<button className="secondary" onClick={()=>setNativeOauth(row)}>重新授权</button>:null}</div>,
  });
  const groupedView=(group:ManagedCredential[]):AccountListRow=>{
    const first=group[0]!;if(group.length===1)return ordinaryView(first);
    const active=group.filter((row)=>ordinaryStatus(row)!=="disabled").length;
    const protocols=[...new Set(group.flatMap((row)=>row.connections.map((c)=>protocolName(c.api_format))))];
    return {...ordinaryView(first),runtime:<AccountRuntimeSummary snapshot={runtimeSnapshot} ids={group.map(row=>row.credential.id)}/>,plan:new Set(group.map(row=>row.plan)).size===1?first.plan:null,source:undefined,authentication:`${group.length} 份授权`,
      selected:group.every((row)=>selection.has(`ordinary:${row.credential.id}`)),onSelect:selecting?()=>toggle(group.map((row)=>`ordinary:${row.credential.id}`)):undefined,
      status:<StatusBadge status={active?"active":"disabled"}>{active} / {group.length} 份已启用</StatusBadge>,
      connection:<button className="account-connection-link" onClick={()=>setAuthorizations(group)}>{protocols.join(" · ")||"未连接接口"}<span className="entity-meta">查看各份授权的连接</span></button>,
      actions:<button className="secondary" onClick={()=>setAuthorizations(group)}>管理授权</button>};
  };
  return <section className="accounts-page">
    <header className="page-head"><div><h2>账号管理</h2><p className="page-description">以账号身份为主线，查看授权、接口与运行状态。</p></div>
      <div className="page-actions">
        <button onClick={() => update("add", "account")}>授权 / 导入账号</button>

      </div>
    </header>
    {navigation}
    {inventory.data&&!inventory.isError?<div className="account-inventory-summary" aria-label="账号目录摘要"><span><strong>{inventory.data.pages[0]?.total.toLocaleString()}</strong>匹配授权</span><span><strong>{Object.keys(inventory.data.pages[0]?.category_totals??{}).filter(key=>(inventory.data?.pages[0]?.category_totals[key]??0)>0).length}</strong>账号类别</span><span><strong>{inventory.data.pages[0]?.unobserved_plan_total.toLocaleString()}</strong>套餐未观测</span></div>:null}
    {notice ? <p role="status">{notice}</p> : null}
    {selecting?<div className="account-batch-toolbar" aria-label="批量账号操作"><span>已选 {selectedTargets.length} / 20 份授权</span><div className="page-actions">{(["enable","disable","remove"] as const).map((action)=><button key={action} className="secondary" disabled={!selectedTargets.length} onClick={()=>setBatch({targets:selectedTargets,action})}>{({enable:"启用",disable:"停用",remove:"移除"})[action]}</button>)}<button className="secondary" disabled={!selection.size} onClick={()=>setSelection(new Set())}>清除选择</button></div></div>:null}
    <div className="account-status-tabs" role="group" aria-label="授权状态筛选">
      {([["","全部授权"],["reauth_required","需要授权"],["enabled","已启用"],["disabled","已停用"]] as const).map(([value,label])=><button key={value} type="button" aria-pressed={selectedStatus===value} onClick={()=>update("status",value)}>{label}</button>)}
    </div>
    <div className="account-directory-toolbar">
      <label className="account-search"><span className="sr-only">搜索账号</span><input aria-label="搜索账号" placeholder="搜索邮箱、用户名、电话或渠道" value={searchInput} onChange={(e)=>setSearchInput(e.target.value)} /></label>
      <label><span className="sr-only">账号类别</span><select aria-label="账号类别" value={selectedCategory} onChange={event=>update("category",event.target.value)}><option value="">全部渠道</option>{accountGroups.map(group=><option key={group.id} value={group.id}>{group.name}</option>)}</select></label>

      <label><span className="sr-only">账号套餐</span><select aria-label="账号套餐" value={withoutPlan?"none":plan?`plan:${plan}`:""} onChange={event=>{setSelection(new Set());const next=new URLSearchParams(params);next.delete("plan");next.delete("without_plan");if(event.target.value==="none")next.set("without_plan","true");else if(event.target.value.startsWith("plan:"))next.set("plan",event.target.value.slice(5));setParams(next,{replace:true});}}><option value="">全部套餐</option><option value="none">未观测套餐{inventory.data?`（${inventory.data.pages[0]?.unobserved_plan_total??0}）`:""}</option>{Object.entries(inventory.data?.pages[0]?.plan_totals??{}).map(([label,count])=><option key={label} value={`plan:${label}`}>{label}（{count}）</option>)}</select></label>
      <label><span className="sr-only">账号排序</span><select aria-label="账号排序" value={sort} onChange={e=>update("sort",e.target.value)}><option value="name">身份 A–Z</option><option value="name_desc">身份 Z–A</option><option value="provider">按渠道</option></select></label>
      <div className="account-directory-tools"><button className="secondary" aria-pressed={selecting} onClick={()=>{setSelecting(!selecting);setSelection(new Set());}}>{selecting?"结束选择":"批量管理"}</button><button className="secondary" disabled={inventory.isFetching} onClick={() => void refresh()}>刷新快照</button></div>
      {provider?<button className="secondary" onClick={()=>update("provider","")}>清除提供商筛选</button>:null}
    </div>
    {error?<div role="alert" className="empty-state">{error.message}<button onClick={()=>void refresh()}>重新读取账号</button></div>:null}
    {inventory.isPending?<p role="status">正在读取账号…</p>:!inventory.isError&&inventory.data?.pages[0]?.total===0?<p className="empty-state">没有匹配的账号。</p>:null}
    <div className="account-directory">
      {accountGroups.filter((group)=>(!selectedCategory||selectedCategory===group.id)&&directory.some(row=>row.category===group.id)).map((group)=>{
        const ordinary=rows.filter((row)=>(row.category??"api")===group.id);
        const identities=groupManagedIdentities(ordinary);
        const grok=provider?[]:nativeRows;
        const loading=group.id==="grok"?native.isPending:!!context&&inventory.isPending;
        const failed=group.id==="grok"?native.isError:inventory.isError;
        return <section className="account-group" key={group.id} aria-label={`${group.name} 账号`}>
          <header className="account-directory-head"><div><h3>{group.name}</h3><span className="entity-meta">{group.description}</span></div><span className="account-group-count">{loading?"读取中":failed?"读取失败":`${inventory.data?.pages[0]?.category_totals[group.id]??0} 份授权`}</span></header>
          {loading?<p className="account-group-empty">读取账号…</p>:failed?<p className="account-group-empty">暂时无法显示此类别</p>:group.id==="grok"?
            (["grok_web","grok_console","grok_build"] as const).filter(channel=>grok.some(row=>row.provider===channel)).map((channel)=><section className="account-subgroup" key={channel} aria-label={nativeNames[channel]}><h4>{nativeNames[channel]}</h4><AccountList rows={grok.filter((row)=>row.provider===channel).map(nativeView)} /></section>):
            <AccountList rows={identities.map(groupedView)} />}
        </section>;
      })}
    </div>
    <div className="data-footer account-directory-footer"><span>已显示 {directory.length} / {inventory.data?.pages[0]?.total??"…"} 份匹配授权</span><div className="page-actions">
      {inventory.hasNextPage?<button className="secondary" disabled={inventory.isFetchingNextPage||inventory.isError} onClick={()=>void inventory.fetchNextPage()}>加载更多账号</button>:null}
    </div></div>
    {authorizations?<Sheet layout="inspector" title="关联授权" description="同一身份可以保留多份独立授权；操作只影响明确选择的一份。" onEscape={()=>setAuthorizations(undefined)} footer={<SheetDismissButton>关闭</SheetDismissButton>}>
      <h3>{accountName(authorizations[0]?.identity)}</h3><p>该身份有 {authorizations.length} 份授权，分别保留连接和状态。操作只影响所选授权。</p>
      <div className="account-grants">{authorizations.map((row)=><section key={row.credential.id} className="account-grant" aria-label="授权记录">
        <div className="page-actions"><strong>{row.provider} · {row.authentication==="oauth"||row.credential.kind==="oauth_json"?"OAuth 授权":"渠道凭据"}</strong>{ordinaryView(row).status}</div>
        <p>{row.connections.length?row.connections.map((c)=>`${protocolName(c.api_format)}${c.host?` · ${c.host}`:""}`).join(" / "):"未连接接口"}</p>
        {row.plan?<p className="entity-meta">套餐 · {row.plan}</p>:null}
        {accountSource(row.credential.id)?<p className="entity-meta">来源 · {accountSource(row.credential.id)}</p>:null}
        <IdentityDetails entries={[["账号",row.credential.id,accountName(row.identity)],["提供商",row.credential.upstream_id,row.provider]]}/>
        {actions(row)}
      </section>)}</div>
    </Sheet>:null}
    {connections?<Sheet layout="inspector" title="接口连接" description="接口定义请求协议与地址；账号池只决定该账号何时参与调度。" onEscape={()=>setConnections(undefined)} footer={<SheetDismissButton>关闭</SheetDismissButton>}>
      <p>{accountName(connections.identity,connections.credential.id)??"未提供账号身份"}</p>
      <p className="muted">网关通过这些接口使用该账号。不同接口可以支持不同请求格式；配置已启用不代表当前正在使用。</p>
      {connections.binding_count===0?<p>尚未连接到接口，暂不用于请求。</p>:<ul className="account-connections">{connections.connections.map((connection)=><li key={connection.id}><strong>{protocolName(connection.api_format)}</strong><span>{connection.host??"未提供主机信息"}</span><StatusBadge status={connection.enabled?"active":"disabled"}>{connection.enabled?"配置已启用":"配置已停用"}</StatusBadge></li>)}</ul>}
      {connections.binding_count>connections.connections.length?<p>仅预览前 {connections.connections.length} 个连接，共 {connections.binding_count} 个。</p>:null}
      <Link to={`/upstreams?upstream_id=${encodeURIComponent(connections.credential.upstream_id)}`} onClick={()=>setConnections(undefined)}>管理接口连接</Link>
    </Sheet>:null}
    {nativeDetail?<NativeAccountDialog account={nativeDetail} onClose={()=>setNativeDetail(undefined)} onAuthorize={()=>{setNativeOauth(nativeDetail);setNativeDetail(undefined);}} onChanged={(notice)=>{setNativeDetail(undefined);setNotice(notice);void refresh();void client.resetQueries({queryKey:["accounts"]});}}/>:null}
    {nativeOauth?<GrokDeviceWizard name={accountName(nativeOauth.identity,nativeOauth.import_batch_id)??"Grok Build 账号"} target={{account_id:nativeOauth.id,revision:nativeOauth.revision}} onClose={()=>{setNativeOauth(undefined);void refresh();}} />:null}
    {["account", "api-key"].includes(params.get("add") ?? "") ? <AddAccountDialog onClose={() => update("add", "")} onCreated={(notice) => {update("add", ""); setNotice(notice??"账号已保存。"); void refresh();}} /> : null}
    {detail ? <CredentialSheet plan={rows.find(row=>row.credential.id===detail)?.plan} credentialId={detail} accountName={accountName(rows.find((row)=>row.credential.id===detail)?.identity)} providerName={rows.find((row)=>row.credential.id===detail)?.provider} category={rows.find((row)=>row.credential.id===detail)?.category==="codex"?"codex":rows.find((row)=>row.credential.id===detail)?.category==="kimi"?"kimi":undefined} onClose={() => {setDetail(undefined); void refresh();}} /> : null}
    {kiroOauth?<KiroDeviceDialog credentialId={kiroOauth.credential.id} providerId={kiroOauth.credential.upstream_id} endpointId="" onClose={()=>setKiroOauth(undefined)} onComplete={notice=>{setKiroOauth(undefined);setNotice(notice);void refresh();}}/>:null}
    {kimiOauth?<KimiDeviceDialog credentialId={kimiOauth.credential.id} providerId={kimiOauth.credential.upstream_id} onClose={()=>setKimiOauth(undefined)} onComplete={notice=>{setKimiOauth(undefined);setNotice(notice);void refresh();}}/>:null}
    {claudeOauth?<AuthorizationCodeDialog channel="claude" credentialId={claudeOauth.credential.id} providerId={claudeOauth.credential.upstream_id} providerName={accountName(claudeOauth.identity)??"Claude"} endpointId="" onClose={()=>setClaudeOauth(undefined)} onComplete={notice=>{setClaudeOauth(undefined);setNotice(notice);void refresh();}}/>:null}
    {oauth ? <OAuthWizard credentialId={oauth} accountName={accountName(rows.find((row)=>row.credential.id===oauth)?.identity)} onClose={() => {setOauth(undefined); void refresh();}} /> : null}
    {more?<Sheet title="账号操作" description="选择一项维护动作；授权、运行状态和删除会在后续步骤明确确认。" layout="confirm" onEscape={()=>setMore(undefined)} footer={<SheetDismissButton className="secondary">取消</SheetDismissButton>}><h3>{accountName(more.identity)??more.provider}</h3><div className="sheet-actions"><button onClick={()=>{setUpdating(more);setMore(undefined);}}>更新凭据</button><button className="secondary" onClick={()=>startAction(more,more.credential.status==="disabled"?"enable":"disable")}>{more.credential.status==="disabled"?"启用":"停用"}账号</button><button className="danger" onClick={()=>startAction(more,"remove")}>移除授权</button></div></Sheet>:null}
    {updating?<CredentialUpdateDialog account={updating} onClose={()=>setUpdating(undefined)} onSaved={(version)=>{setUpdating(undefined);void refresh();useVersionStore.getState().select(version);}}/>:null}
    {batch?<AccountBatchDialog targets={batch.targets} action={batch.action} onClose={()=>setBatch(undefined)} onCompleted={(message,version)=>{setBatch(undefined);setSelection(new Set());setNotice(message);void refresh();if(version)useVersionStore.getState().select(version);}}/>:null}
  </section>;
}
