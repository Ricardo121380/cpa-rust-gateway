import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { asAppError } from "../../api/errors";
import { StatusBadge } from "../../components/StatusBadge";
import { useNativeAccounts, type NativeAccount } from "../accounts/NativeAccounts";
import { NativeAccountDialog } from "../accounts/NativeAccountDialog";
import { GrokDeviceWizard } from "../accounts/GrokDeviceWizard";
import { accountName, accountSource } from "../accounts/presentation";
import type { ManagedEndpoint } from "../accounts/inventory";
import { nativeAccountsForEndpoints } from "./subresourceModel";

const names = { grok_build: "Grok Build", grok_console: "Grok Console", grok_web: "Grok Web" };

export function NativeProviderAccounts({ endpoints, onAddAccount, onActionActiveChange }: Readonly<{
  endpoints: readonly ManagedEndpoint[]; onAddAccount: () => void; onActionActiveChange: (active: boolean) => void;
}>) {
  const inventory = useNativeAccounts();
  const client = useQueryClient();
  const [detail, setDetail] = useState<NativeAccount>();
  const [authorization, setAuthorization] = useState<NativeAccount>();
  const [notice, setNotice] = useState<string>();
  const active = detail !== undefined || authorization !== undefined;
  useEffect(() => { onActionActiveChange(active); return () => onActionActiveChange(false); }, [active, onActionActiveChange]);
  const refresh = () => {
    for (const key of ["native-accounts", "account-directory", "account-list-runtime", "accounts"])
      void client.resetQueries({ queryKey: [key] });
  };
  const accounts = nativeAccountsForEndpoints(inventory.data?.pages.flatMap(page => page.items) ?? [], endpoints);
  return <section aria-label="渠道账号">
    <h3>渠道账号 <span className="idchip mono">{inventory.isPending || inventory.isError ? "—" : accounts.length}</span></h3>
    <button className="secondary" onClick={onAddAccount}>添加账号</button>
    <p className="stat-sub">账号按渠道参与调度，无需建立普通凭据绑定；保存授权不代表当前可调度。</p>
    {notice ? <p role="status">{notice}</p> : null}
    {inventory.isPending ? <p role="status">读取渠道账号…</p> : inventory.isError ? <p role="alert">{asAppError(inventory.error).message}<button onClick={refresh}>重新读取渠道账号</button></p> : <>
      <div className="subresource-list" aria-label="渠道账号列表">{accounts.map(account => <article className="subresource-row" key={account.id}>
        <div className="subresource-identity"><span className="subresource-label">账号</span><strong>{accountName(account.identity) ?? "未提供账号身份"}</strong><span className="subresource-meta">{names[account.provider]}{accountSource(account.import_batch_id) ? " · 来源 Autoreg" : ""}</span></div>
        <dl className="subresource-facts"><div><dt>接入方式</dt><dd>{account.provider === "grok_build" ? "OAuth 授权" : "SSO 授权"}</dd></div><div><dt>授权状态</dt><dd><StatusBadge status={account.enabled ? account.auth_status : "disabled"}>{!account.enabled || account.auth_status === "disabled" ? "已停用" : account.auth_status === "active" ? "已保存授权" : "需要重新授权"}</StatusBadge></dd></div></dl>
        <div className="subresource-actions"><button className="secondary" onClick={() => setDetail(account)}>详情</button>{account.provider === "grok_build" ? <button className="secondary" onClick={() => setAuthorization(account)}>重新授权</button> : null}</div>
      </article>)}</div>
      {accounts.length === 0 ? <p>{inventory.hasNextPage ? "已加载的账号中暂无此渠道，请继续加载。" : "尚未添加此渠道的账号。"}</p> : null}
      {inventory.hasNextPage ? <button className="secondary" disabled={inventory.isFetchingNextPage} onClick={() => void inventory.fetchNextPage()}>加载更多渠道账号</button> : null}
      {inventory.hasNextPage ? <p className="stat-sub">仅显示已加载的渠道账号，以上数量不是总数。</p> : null}
    </>}
    {detail ? <NativeAccountDialog account={detail} onClose={() => setDetail(undefined)} onAuthorize={() => { setAuthorization(detail); setDetail(undefined); }} onChanged={message => { setNotice(message); setDetail(undefined); refresh(); }} /> : null}
    {authorization ? <GrokDeviceWizard name={accountName(authorization.identity) ?? "Grok Build 账号"} target={{account_id: authorization.id, revision: authorization.revision}} onClose={() => { setAuthorization(undefined); refresh(); }} /> : null}
  </section>;
}
