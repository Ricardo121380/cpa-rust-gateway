import { useState } from "react";
import "./resource-identity.css";
import { isInternalLabel, resourceCode, resourceName, type ResourceKind } from "../utils/resourceNames";

export function ResourceIdentity({ id, kind = "resource", name }: Readonly<{
  id: string; kind?: ResourceKind; name?: string | null;
}>) {
  const internal = isInternalLabel(id) || isInternalLabel(name?.trim() || id);
  return <span className="resource-identity" title={id} data-resource-id={id}>
    <span>{resourceName(id, kind, name)}</span>
    {!internal && name && name !== id ? <small className="resource-original-id">{id}</small> : null}
    {internal ? <small className="resource-code" aria-label={`标识 ${resourceCode(id)}`}>{resourceCode(id)}</small> : null}
  </span>;
}

export function IdentityDetails({ entries }: Readonly<{ entries: ReadonlyArray<readonly [string, string]> }>) {
  const [notice, setNotice] = useState("");
  async function copy(id: string) {
    try { await navigator.clipboard.writeText(id); setNotice("已复制标识"); }
    catch { setNotice("未能复制，请选中文本复制。"); }
  }
  return <details className="identity-details">
    <summary>技术标识</summary>
    <dl>{entries.map(([label, id]) => <div key={label}>
      <dt>{label}</dt><dd><code>{id}</code><button className="secondary" aria-label={`复制${label}`} onClick={() => void copy(id)}>复制</button></dd>
    </div>)}</dl><span role="status">{notice}</span>
  </details>;
}
