import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import type {AccountIdentity} from "./presentation";
import {accountName} from "./presentation";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { safeExternalUrl } from "../upstreams/model";
import { RuntimeApplyNotice } from "./RuntimeApplyNotice";
type View = Readonly<{session_id:string;state:string;user_code:string;verification_uri:string;expires_at_ms:number;retry_at_ms:number;identity:AccountIdentity|null;identity_state:string;runtime_applied?:boolean|null}>;
export function GrokDeviceWizard({name,target,onClose,onComplete}:Readonly<{name:string;target?:{account_id:string;revision:number};onClose:()=>void;onComplete?:()=>void}>) {
  const client=useQueryClient();
  const [session,setSession]=useState<View>();
  const key=["grok-device",session?.session_id];
  const poll=useQuery({queryKey:key,enabled:session?.state==="pending",retry:false,refetchOnWindowFocus:false,refetchOnReconnect:false,
    queryFn:()=>call<View>("pollNativeAccountAuthorization",{path:{session_id:session!.session_id}}),
    refetchInterval:(q)=>q.state.status==="error" ? false : q.state.data && q.state.data.state!=="pending" ? false : Math.max(1000,(q.state.data?.retry_at_ms ?? session?.retry_at_ms ?? Date.now()+5000)-Date.now()),
  });
  const start=useMutation({mutationFn:()=>call<View>("startNativeAccountAuthorization",{body:{...(name?{name}:{}),...(target?{target}:{})}}),onSuccess:setSession});
  const cancel=useMutation({mutationFn:()=>call<View>("cancelNativeAccountAuthorization",{path:{session_id:session!.session_id}}),onSuccess:(value)=>client.setQueryData(key,value)});
  const view=poll.data??session;
  const dismissAuthorization=async()=>{
    if(view?.state!=="pending")return true;
    await client.cancelQueries({queryKey:key});
    try {return (await cancel.mutateAsync()).state!=="complete";} catch {return false;}
  };
  const close=()=>{void client.resetQueries({queryKey:["native-accounts"]});if(view?.state==="complete")onComplete?.();onClose();};
  const labels:Record<string,string>={pending:"等待 Grok 授权",complete:"授权已保存",denied:"授权被拒绝",expired:"授权已过期",cancelled:"已取消授权",failed:"授权通信失败",persistence_conflict:"账号身份或版本冲突，凭据未被覆盖"};
  const error=start.error??poll.error??cancel.error;
  const href=safeExternalUrl(view?.verification_uri);
  const busy=start.isPending||cancel.isPending;
  const footer=!view?<><SheetDismissButton className="secondary" disabled={busy}>取消</SheetDismissButton><button type="button" disabled={busy||start.isError} onClick={()=>start.mutate()}>开始 Grok 授权</button></>:view.state==="pending"?<SheetDismissButton className="secondary" disabled={busy}>取消授权</SheetDismissButton>:<SheetDismissButton disabled={busy}>关闭</SheetDismissButton>;
  return <Sheet title={target?"Grok 重新授权":"添加 Grok 授权账号"} description={target?"仅更新选中账号的官方授权，不改变其他账号或连接。":"在 Grok 官方页面输入设备验证码；保存后会读取授权身份。"} onEscape={close} onBeforeDismiss={dismissAuthorization} busy={busy} blockNavigation={view?.state==="pending"} footer={footer}>
    <div className="operation-summary"><span>Grok Build · 设备授权</span><strong>{target&&name?name:"连接 Grok 账号"}</strong><small>获取验证码 → 官方确认 → 自动保存</small></div>
    {!view?<p>开始后在 Grok 官方页面完成授权；此窗口会保留验证码并读取身份。</p>:<>
      <p role="status" data-native-session-id={view.session_id}>{labels[view.state]??view.state}</p>
      {view.state==="pending"?<>
        <p>在 Grok 授权页面输入此验证码：</p><strong className="authorization-device-code mono">{view.user_code}</strong>
        {href?<a className="button" href={href} target="_blank" rel="noopener noreferrer">打开 Grok 授权页面</a>:null}
        <p className="small muted">有效期至 {new Date(view.expires_at_ms).toLocaleTimeString()}。完成后此处自动更新。</p>
      </>:null}
      {view.state==="complete"?<>
        {view.identity?<p className="authorized-account-identity"><strong>{accountName(view.identity)}</strong></p>:null}
        {view.identity_state==="unavailable"?<p role="status">授权已保存，但暂未取得账号身份。</p>:view.identity_state==="not_provided"?<p role="status">授权服务未返回邮箱、电话或用户名。</p>:null}
        {view.runtime_applied===true?<p>新授权已应用。</p>:<RuntimeApplyNotice onApplied={()=>client.setQueryData(key,{...view,runtime_applied:true})}/>}
      </>:null}
    </>}
    {error?<p role="alert">{asAppError(error).message}</p>:null}
  </Sheet>;
}
