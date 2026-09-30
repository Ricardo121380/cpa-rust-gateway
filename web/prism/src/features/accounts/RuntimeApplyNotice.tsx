import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError, requiresWriteReconciliation } from "../../api/errors";
import { useMessages } from "../../i18n/messages";

export function RuntimeApplyNotice({onApplied,onBusyChange,disabled=false}:Readonly<{onApplied:()=>void;onBusyChange?:(busy:boolean)=>void;disabled?:boolean}>) {
  const t=useMessages().runtimeApplication;
  const [needsReview,setNeedsReview]=useState(false);
  const [appliedAt,setAppliedAt]=useState<number>();
  const apply=useMutation({mutationFn:()=>call<{runtime_applied:boolean}>("applyRuntimeConfiguration"),onSuccess:(result)=>{if(result.runtime_applied){setAppliedAt(Date.now());onApplied();}},onError:cause=>setNeedsReview(requiresWriteReconciliation(cause))});
  const review=useMutation({mutationFn:()=>call<{accepting_requests:boolean|null}>("getSystemInformation"),onSuccess:()=>setNeedsReview(false)});
  const busy=apply.isPending||review.isPending;
  useEffect(()=>{onBusyChange?.(busy);return()=>onBusyChange?.(false);},[busy,onBusyChange]);
  return <div role="status"><p>{needsReview?t.uncertain:appliedAt?t.confirmed:review.isSuccess?t.uncertaintyRemains:t.saved}</p>
    {appliedAt?<p>{t.previous}：{new Date(appliedAt).toLocaleString()}</p>:null}
    {!needsReview&&!appliedAt&&!review.isSuccess?<p>{t.blocked}</p>:null}
    {apply.isError?<p role="alert">{asAppError(apply.error).code} · {asAppError(apply.error).message}</p>:apply.data?.runtime_applied===false?<p role="alert">{t.failed}</p>:null}
    {review.data?<p>{t.observed}：{review.data.accepting_requests===true?t.accepting:review.data.accepting_requests===false?t.paused:t.unknown}</p>:null}
    {review.isError?<p role="alert">{t.reviewFailed}：{asAppError(review.error).message}</p>:null}
    {disabled ? <p>{t.pendingRead}</p> : null}
    {needsReview?<button type="button" className="secondary" disabled={busy} onClick={()=>review.mutate()}>{review.isPending?t.reviewing:t.review}</button>:null}
    <button type="button" className="primary" disabled={busy || disabled || needsReview} onClick={()=>apply.mutate()}>{apply.isPending?t.applying:t.apply}</button>
  </div>;
}
