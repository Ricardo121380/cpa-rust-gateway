import type {ReactNode} from "react";
export type AccountListRow = Readonly<{key:string;name:string|undefined;source:string|undefined;provider:string;authentication:string;plan?:string|null;planSource?:string|null;status:ReactNode;runtime?:ReactNode;connection:ReactNode;actions:ReactNode;selected?:boolean;onSelect?:()=>void}>;
export function AccountList({rows}:Readonly<{rows:readonly AccountListRow[]}>) {
  if (!rows.length) return <div className="account-group-empty">暂无账号</div>;
  return <div className="account-list-wrap"><table className="account-list"><thead><tr><th>账号身份</th><th>渠道</th><th>授权状态</th><th>运行与额度</th><th>接口连接</th><th><span className="sr-only">操作</span></th></tr></thead>
    <tbody>{rows.map((row)=><tr key={row.key} data-account-key={row.key}>
      <td className="account-list-identity">{row.onSelect?<input type="checkbox" className="account-row-checkbox" aria-label={`选择 ${row.name??row.provider+"账号"}`} checked={row.selected??false} onChange={row.onSelect}/>:null}<span className="account-identity-layout"><span className="account-identity-avatar" aria-hidden="true">{row.name?.slice(0,1).toUpperCase()??"?"}</span><span><strong className={row.name?undefined:"muted"}>{row.name??"未提供账号身份"}</strong>{row.source?<span className="account-source">来源 · {row.source}</span>:null}{!row.name?<span className="entity-meta">授权服务或导入资料未返回身份</span>:null}</span></span></td>
      <td data-label="渠道"><span>{row.provider}</span><span className="entity-meta">{row.authentication}</span>{row.plan?<span className="entity-meta" title={row.planSource==="provider_subscription"?"渠道套餐观测":row.planSource==="token_claim"||row.planSource==="signed_token"?"令牌中的套餐声明":"导入资料中的套餐声明"}>套餐 · {row.plan}</span>:null}</td>
      <td data-label="授权状态">{row.status}</td><td data-label="运行与额度">{row.runtime??"未观测"}</td><td data-label="接口连接">{row.connection}</td><td className="account-list-actions">{row.actions}</td>
    </tr>)}</tbody>
  </table></div>;
}
