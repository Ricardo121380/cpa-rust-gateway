import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";

export function RuntimeApplyNotice({onApplied,onBusyChange,disabled=false}:Readonly<{onApplied:()=>void;onBusyChange?:(busy:boolean)=>void;disabled?:boolean}>) {
  const apply=useMutation({mutationFn:()=>call<{runtime_applied:boolean}>("applyRuntimeConfiguration"),onSuccess:(result)=>{if(result.runtime_applied)onApplied();}});
  useEffect(()=>{onBusyChange?.(apply.isPending);return()=>onBusyChange?.(false);},[apply.isPending,onBusyChange]);
  return <div role="status"><p>修改已保存，运行配置暂未应用。</p>
    <p role="alert">运行重建失败已使新请求被全局阻断，其他账号和渠道的新请求也会受影响。已保存的授权保留；请重新应用运行配置以恢复服务。</p>
    {apply.isError?<p role="alert">{asAppError(apply.error).message}</p>:apply.data?.runtime_applied===false?<p role="alert">暂时无法应用，请稍后重试。</p>:null}
    {disabled ? <p>当前读取未完成，先重新读取服务状态后再应用。</p> : null}
    <button type="button" className="primary" disabled={apply.isPending || disabled} onClick={()=>apply.mutate()}>应用运行配置</button>
  </div>;
}
