import { useState, type InputHTMLAttributes } from "react";
import "./resource-identity.css";
import { resourceName, referenceKind, type ResourceKind } from "../utils/resourceNames";

export function ResourceIdentity({ id, kind = "resource", name }: Readonly<{
  id: string; kind?: ResourceKind; name?: string | null;
}>) {
  return <span className="resource-identity" data-resource-id={id}>
    <span>{resourceName(id, kind, name)}</span>
  </span>;
}

export function IdentityDetails({ entries }: Readonly<{ entries: ReadonlyArray<readonly [string, string, (string | null)?]> }>) {
  const [notice, setNotice] = useState("");
  async function copy(id: string) {
    try { await navigator.clipboard.writeText(id); setNotice("已复制内部引用"); }
    catch { setNotice("未能复制，请检查浏览器的剪贴板权限。"); }
  }
  return <details className="identity-details">
    <summary>关联信息</summary>
    <dl>{entries.map(([label, id, name]) => <div key={label}>
      <dt>{label.replace(/\s*ID$/u, "")}</dt><dd><ResourceIdentity id={id} kind={referenceKind(label)} name={name} /><button className="secondary" aria-label={`复制${label}内部引用`} onClick={() => void copy(id)}>复制内部引用</button></dd>
    </div>)}</dl><span role="status">{notice}</span>
  </details>;
}

/** Preserve the original form value while showing a readable immutable resource. */
export function ResourceIdInput({kind="resource",displayName,...props}: InputHTMLAttributes<HTMLInputElement> & {kind?:ResourceKind;displayName?:string|null}) {
  if (!props.readOnly && !props.disabled) return <input {...props} />;
  const value=String(props.value??props.defaultValue??"");
  return <><input type="hidden" name={props.name} value={value} disabled={props.disabled}/><ResourceIdentity id={value} kind={kind} name={displayName}/></>;
}
