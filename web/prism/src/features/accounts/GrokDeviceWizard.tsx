import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { safeExternalUrl } from "../upstreams/model";
type View = Readonly<{session_id:string;state:string;user_code:string;verification_uri:string;expires_at_ms:number;retry_at_ms:number}>;
export function GrokDeviceWizard({name,target,onClose,onComplete}:Readonly<{name:string;target?:{account_id:string;revision:number};onClose:()=>void;onComplete?:()=>void}>) {
  const client=useQueryClient();
  const [session,setSession]=useState<View>();
  const key=["grok-device",session?.session_id];
  const poll=useQuery({queryKey:key,enabled:session?.state==="pending",retry:false,refetchOnWindowFocus:false,refetchOnReconnect:false,
    queryFn:()=>call<View>("pollNativeAccountAuthorization",{path:{session_id:session!.session_id}}),
    refetchInterval:(q)=>q.state.status==="error" ? false : q.state.data && q.state.data.state!=="pending" ? false : Math.max(1000,(q.state.data?.retry_at_ms ?? session?.retry_at_ms ?? Date.now()+5000)-Date.now()),
  });
  const start=useMutation({mutationFn:()=>call<View>("startNativeAccountAuthorization",{body:{name,...(target?{target}:{})}}),onSuccess:setSession});
  const cancel=useMutation({mutationFn:()=>call<View>("cancelNativeAccountAuthorization",{path:{session_id:session!.session_id}}),onSuccess:(value)=>client.setQueryData(key,value)});
  const view=poll.data??session;
  const close=()=>{if(view?.state==="pending") cancel.mutate();else{void client.resetQueries({queryKey:["native-accounts"]});if(view?.state==="complete")onComplete?.();onClose();}};
  const labels:Record<string,string>={pending:"等待 Grok 授权",complete:"授权已保存",denied:"授权被拒绝",expired:"授权已过期",cancelled:"已取消授权",failed:"授权通信失败",persistence_conflict:"账号身份或版本冲突，凭据未被覆盖"};
  const error=start.error??poll.error??cancel.error;
  const href=safeExternalUrl(view?.verification_uri);
  return <Sheet title={target?"Grok 重新授权":"添加 Grok 授权账号"} onEscape={start.isPending||cancel.isPending?undefined:close}>
    <p>{name}</p>
    {!view?<button disabled={start.isPending} onClick={()=>start.mutate()}>开始 Grok 授权</button>:<>
      <p role="status" data-native-session-id={view.session_id}>{labels[view.state]??view.state}</p>
      {view.state==="pending"?<>
        <p>在 Grok 授权页面输入此验证码：</p><p className="mono">{view.user_code}</p>
        {href?<a className="button" href={href} target="_blank" rel="noopener noreferrer">打开 Grok 授权页面</a>:null}
        <p className="small muted">有效期至 {new Date(view.expires_at_ms).toLocaleTimeString()}。完成后此处自动更新。</p>
      </>:null}
      {view.state==="complete"?<p>账号已保存，新授权将在网关重新载入配置后用于请求。</p>:null}
    </>}
    {error?<p role="alert">{asAppError(error).message}</p>:null}
    <div className="sheet-actions"><button className="secondary" disabled={start.isPending||cancel.isPending} onClick={close}>{view?.state==="pending"?"取消授权":"关闭"}</button></div>
  </Sheet>;
}
