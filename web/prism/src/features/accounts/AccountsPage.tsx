import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { Sheet } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { beginConfigurationEdit } from "../config-versions/beginEdit";
import { useVersionStore } from "../config-versions/versionStore";
import { CredentialSheet } from "../upstreams/CredentialSheet";
import { OAuthWizard } from "../upstreams/OAuthWizard";
import { NativeAccounts } from "./NativeAccounts";
import { AddAccountDialog } from "./AddAccountDialog";
import { AccountRuntimePanel } from "./AccountRuntimePanel";
import { useManagedInventory, type ManagedCredential } from "./inventory";

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
  const inventory = useManagedInventory("credentials", provider, search);
  const rows = inventory.data?.pages.flatMap((p) => p.items) ?? [];
  const [detail, setDetail] = useState<string>();
  const [oauth, setOauth] = useState<string>();
  const [status, setStatus] = useState<ManagedCredential>();
  const [notice, setNotice] = useState<string>();
  const refresh = () => Promise.all([client.resetQueries({queryKey: ["managed-inventory"]}),client.resetQueries({queryKey: ["native-accounts"]})]);
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
              <button className="secondary" onClick={() => setDetail(row.credential.id)}>详情</button>
              {row.credential.kind === "oauth_json" ? <button className="secondary" onClick={() => setOauth(row.credential.id)}>重新授权</button> : null}
              {context?.status === "draft" ? <button className="secondary" onClick={() => {change.reset(); setStatus(row);}}>{row.credential.status === "disabled" ? "启用" : "停用"}</button> : null}
            </div>;
  return <section className="accounts-page">
    <header className="page-head"><div><h2>账号管理</h2><p>管理账号及其使用位置</p></div>
      <div className="page-actions">
        <button disabled={edit.isPending} onClick={() => edit.mutate("create")}>添加账号</button>
        {context?.status !== "draft" ? <button disabled={edit.isPending} onClick={() => edit.mutate("edit")}>编辑账号配置</button> : null}
        <button className="secondary" onClick={() => void refresh()}>刷新账号</button>
      </div>
    </header>
    {edit.isError ? <p role="alert">{asAppError(edit.error).message}</p> : null}
    {notice ? <p role="status">{notice}</p> : null}
    <div className="data-panel">
      <div className="data-toolbar">
        <input aria-label="搜索账号" placeholder="搜索账号或提供商" value={search} onChange={(e) => update("q", e.target.value)} />
        {provider ? <button className="secondary" onClick={() => update("provider", "")}>清除提供商筛选</button> : null}
      </div>
      {error ? <div className="empty-state" role="alert">{error.message}<button onClick={() => void refresh()}>重新读取</button></div> :
        context === undefined ? <div className="empty-state">开始编辑账号配置以接入第一个账号。</div> :
        inventory.isPending ? <div className="empty-state">读取账号…</div> : rows.length === 0 ? <div className="empty-state">{search || provider ? "没有匹配的账号" : "尚未添加账号"}</div> :
        <><div className="tablewrap managed-account-table"><table><thead><tr><th>账号</th><th>提供商</th><th>认证方式</th><th>启停状态</th><th>使用位置</th><th>操作</th></tr></thead>
          <tbody>{rows.map((row) => <tr key={row.credential.id}>
            <td><ResourceIdentity id={row.credential.id} kind="account" /></td>
            <td><ResourceIdentity id={row.credential.upstream_id} kind="upstream" /></td>
            <td>{row.credential.kind === "oauth_json" ? "Codex OAuth" : row.credential.kind === "bearer" ? "渠道凭据" : row.credential.kind}</td>
            <td><StatusBadge status={row.credential.status}>{row.credential.status === "active" ? "已启用" : "已停用"}</StatusBadge></td>
            <td><Link to={`/upstreams?upstream_id=${encodeURIComponent(row.credential.upstream_id)}`}>{row.binding_count === 0 ? "未绑定" : `${row.binding_count} 个端点`}</Link></td>
            <td>{actions(row)}</td>
          </tr>)}</tbody></table></div>
        <div className="managed-account-cards">{rows.map((row) => <article key={row.credential.id}>
          <header><strong><ResourceIdentity id={row.credential.id} kind="account" /></strong><StatusBadge status={row.credential.status}>{row.credential.status === "active" ? "已启用" : "已停用"}</StatusBadge></header>
          <p className="entity-meta"><ResourceIdentity id={row.credential.upstream_id} kind="upstream" /> · {row.credential.kind === "oauth_json" ? "Codex OAuth" : "渠道凭据"}</p>
          <p><Link to={`/upstreams?upstream_id=${encodeURIComponent(row.credential.upstream_id)}`}>{row.binding_count === 0 ? "未绑定端点" : `${row.binding_count} 个端点`}</Link></p>
          {actions(row)}
        </article>)}</div></>}
      <div className="data-footer"><span>已加载 {rows.length} 个账号{inventory.hasNextPage ? " · 还有更多" : ""}</span>
        {inventory.hasNextPage ? <button className="secondary" disabled={inventory.isFetchingNextPage || inventory.isError} onClick={() => void inventory.fetchNextPage()}>加载更多账号</button> : null}
      </div>
    </div>
    <NativeAccounts search={search} />
    {["account", "api-key"].includes(params.get("add") ?? "") && context?.status === "draft" ? <AddAccountDialog onClose={() => update("add", "")} onCreated={() => {update("add", ""); setNotice("账号已保存。"); void refresh();}} /> : null}
    {detail ? <CredentialSheet credentialId={detail} onClose={() => {setDetail(undefined); void refresh();}} /> : null}
    {oauth ? <OAuthWizard credentialId={oauth} onClose={() => {setOauth(undefined); void refresh();}} /> : null}
    {status ? <Sheet title={status.credential.status === "disabled" ? "启用账号" : "停用账号"} onEscape={() => !change.isPending && setStatus(undefined)}>
      <p><ResourceIdentity id={status.credential.id} kind="account" /></p>
      <p>更改将在配置应用后生效。凭据和历史记录保留。</p>
      {change.isError ? <p role="alert">{asAppError(change.error).message}</p> : null}
      <div className="sheet-actions"><button className="secondary" disabled={change.isPending} onClick={() => setStatus(undefined)}>取消</button><button disabled={change.isPending} onClick={() => change.mutate(status)}>确认更改</button></div>
    </Sheet> : null}
  </section>;
}
