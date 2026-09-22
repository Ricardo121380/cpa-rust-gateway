import type {ReactNode} from "react";

/** One identity anchor for both managed credentials and native channel accounts. */
export function AccountIdentityHeader({name,provider,method,status}:Readonly<{
  name?:string|null;provider:string;method:string;status?:ReactNode;
}>) {
  return <header className="account-inspector-identity">
    <span className="account-identity-avatar" aria-hidden="true">{(name||provider).slice(0,1).toUpperCase()}</span>
    <div><h3>{name||"未提供账号身份"}</h3><p>{provider}<span aria-hidden="true"> · </span>{method}</p></div>
    {status?<div className="account-inspector-status">{status}</div>:null}
  </header>;
}
