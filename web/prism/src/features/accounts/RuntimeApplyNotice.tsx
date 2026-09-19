import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";

export function RuntimeApplyNotice({onApplied,onBusyChange}:Readonly<{onApplied:()=>void;onBusyChange?:(busy:boolean)=>void}>) {
  const apply=useMutation({mutationFn:()=>call<{runtime_applied:boolean}>("applyRuntimeConfiguration"),onSuccess:(result)=>{if(result.runtime_applied)onApplied();}});
  useEffect(()=>{onBusyChange?.(apply.isPending);return()=>onBusyChange?.(false);},[apply.isPending,onBusyChange]);
  return <div role="status"><p>修改已保存，运行配置暂未应用。</p>
    {apply.isError?<p role="alert">{asAppError(apply.error).message}</p>:apply.data?.runtime_applied===false?<p role="alert">暂时无法应用，请稍后重试。</p>:null}
    <button type="button" disabled={apply.isPending} onClick={()=>apply.mutate()}>应用运行配置</button>
  </div>;
}
