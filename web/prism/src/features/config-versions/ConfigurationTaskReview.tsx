import {useMutation} from "@tanstack/react-query";
import {useState} from "react";
import {asAppError} from "../../api/errors";
import {referenceText} from "../../utils/resourceNames";
import {changeLabel,fieldLabel} from "./pendingChanges";
import type {ConfigurationTask,ConfigurationTaskReview as Review} from "./configurationTask";
import type {ConfigVersionSummary} from "./versionStore";

/** An explicit in-task review uses the complete server diff and frozen revisions. */
export function ConfigurationTaskReview({task,onApplied,onBusyChange,onReviewed}:Readonly<{task:ConfigurationTask;onApplied:(version:ConfigVersionSummary)=>void;onBusyChange?:(busy:boolean)=>void;onReviewed?:(review:Review)=>Promise<ConfigVersionSummary>}>) {
  const [restorationConfirmed,setRestorationConfirmed]=useState(false);
  const [confirmed,setConfirmed]=useState(false);
  const preview=useMutation({mutationFn:()=>task.preview(),onMutate:()=>{setConfirmed(false);setRestorationConfirmed(false);}});
  const apply=useMutation({mutationFn:async()=>{if(!preview.data||!confirmed)throw new Error("请先核对完整清单。");return onReviewed?onReviewed(preview.data):task.apply(preview.data,restorationConfirmed);},onMutate:()=>onBusyChange?.(true),onSuccess:onApplied,onSettled:()=>{onBusyChange?.(false);setConfirmed(false);preview.reset();}});
  return <section aria-label="本次配置核对">
    <p role="status">已保存的修改可保留待应用，也可在这里核对完整清单后应用。授权保存不会因此撤销。</p>
    <button type="button" className="secondary" disabled={preview.isPending||apply.isPending} onClick={()=>preview.mutate()}>核对合并清单</button>
    {preview.data?<>
      <details><summary>本次操作前的修改 · {preview.data.priorChanges.length} 项</summary><ul>{preview.data.priorChanges.map(row=><li key={`${row.resource_kind}:${row.resource_key}`}>{changeLabel[row.change]} · {referenceText(row.resource_key)} · {row.changed_fields.map(fieldLabel).join("、")}</li>)}</ul></details>
      <h4>将应用的完整清单 · {preview.data.changes.length} 项</h4>
      {!preview.data.active?<p>首次配置，没有活动版本可供比较；校验后应用这份配置。</p>:null}
      <ul>{preview.data.changes.map(row=><li key={`${row.resource_kind}:${row.resource_key}`}>{changeLabel[row.change]} · {referenceText(row.resource_key)}<span className="entity-meta">{row.changed_fields.map(fieldLabel).join("、")||"资源整体变化"}</span></li>)}</ul>
      {preview.data.restoration.accounts.length?<section><h4>将恢复的已删除账号</h4><ul>{preview.data.restoration.accounts.map(account=><li key={`${account.upstream_id}:${account.credential_id}`}>{account.credential_id} · {account.upstream_id} · 授权 revision {account.credential_revision}</li>)}</ul><label className="check-row"><input type="checkbox" checked={restorationConfirmed} onChange={event=>setRestorationConfirmed(event.target.checked)}/>确认恢复这些历史账号授权</label></section>:null}
      <label className="check-row"><input type="checkbox" checked={confirmed} onChange={event=>setConfirmed(event.target.checked)} disabled={apply.isPending}/>已核对已有修改与本次修改，{onReviewed?"确认继续处理已保存账号":"确认一起应用"}</label>
      <button type="button" className="primary" disabled={!confirmed||apply.isPending||!!preview.data.restoration.accounts.length&&!restorationConfirmed} onClick={()=>apply.mutate()}>{onReviewed?"按这份清单继续连接":"校验并应用这份清单"}</button>
    </>:null}
    {preview.isError||apply.isError?<p role="alert">{asAppError(preview.error??apply.error).message}。原保存回执保留；请重新核对，不要重放写入。</p>:null}
  </section>;
}
