import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import type { ConfigVersionSummary } from "./versionStore";

/** A failed multi-resource operation remains inspectable; writes are never replayed. */
export function ConfigurationTaskNotice({workingId,error,onReview}:Readonly<{workingId?:string;error?:unknown;onReview:(version:ConfigVersionSummary)=>void}>) {
  const review=useMutation({mutationFn:()=>call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:workingId!}}),onSuccess:onReview});
  if(!error)return null;
  return <div role="alert"><p>{asAppError(error).message}</p>{workingId?<button type="button" className="secondary" disabled={review.isPending} onClick={()=>review.mutate()}>查看待应用的修改</button>:null}{review.isError?<p>{asAppError(review.error).message}</p>:null}</div>;
}
