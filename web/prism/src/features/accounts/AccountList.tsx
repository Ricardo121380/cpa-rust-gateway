import type {ReactNode} from "react";
export type AccountListRow = Readonly<{key:string;name:string|undefined;source:string|undefined;provider:string;authentication:string;status:ReactNode;connection:ReactNode;actions:ReactNode}>;
export function AccountList({rows}:Readonly<{rows:readonly AccountListRow[]}>) {
  if (!rows.length) return <div className="account-group-empty">暂无账号</div>;
  return <div className="account-list-wrap"><table className="account-list"><thead><tr><th>账号身份</th><th>渠道</th><th>状态</th><th>接口连接</th><th><span className="sr-only">操作</span></th></tr></thead>
    <tbody>{rows.map((row)=><tr key={row.key} data-account-key={row.key}>
      <td className="account-list-identity"><strong className={row.name?undefined:"muted"}>{row.name??"未提供账号身份"}</strong>{row.source?<span className="account-source">来源 · {row.source}</span>:null}{!row.name?<span className="entity-meta">凭据中没有邮箱、电话或用户名</span>:null}</td>
      <td data-label="渠道"><span>{row.provider}</span><span className="entity-meta">{row.authentication}</span></td>
      <td data-label="状态">{row.status}</td><td data-label="接口连接">{row.connection}</td><td className="account-list-actions">{row.actions}</td>
    </tr>)}</tbody>
  </table></div>;
}
