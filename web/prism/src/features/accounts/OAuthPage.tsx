import {useQuery, useQueryClient} from "@tanstack/react-query";
import {useState} from "react";
import {Link} from "react-router-dom";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {useVersionStore} from "../config-versions/versionStore";
import {AuthorizationCodeDialog} from "./AuthorizationCodeDialog";
import {KimiDeviceDialog} from "./KimiDeviceDialog";
import {KiroDeviceDialog} from "./KiroDeviceDialog";
import {GrokDeviceWizard} from "./GrokDeviceWizard";

type Channel=Readonly<{id:string;name:string;authorization_available:boolean;authorization_flow:string}>;
export function OAuthPage() {
 const client=useQueryClient();const context=useVersionStore(s=>s.context);
 const [channel,setChannel]=useState<string>();const [provider,setProvider]=useState("");const [notice,setNotice]=useState<string>();
 const channels=useQuery({queryKey:["account-channels"],queryFn:()=>call<readonly Channel[]>("listAccountChannels"),retry:false});
 const targets=useQuery({queryKey:["oauth-kimi-targets",context?.configVersionId,context?.revision],enabled:!!context,queryFn:()=>call<readonly {id:string;name:string;kind:string}[]>("listUpstreams",{},{versionScoped:true}),retry:false});
 const kimi=targets.data?.filter(row=>row.kind==="kimi-coding")??[];
 const complete=(message?:string)=>{setChannel(undefined);setNotice(message??"授权流程已处理，请在账号管理查看保存与应用回执。");void client.invalidateQueries({queryKey:["account-directory"]});};
 return <section><header className="page-head"><div><h2>OAuth 授权</h2><p className="page-description">选择渠道完成官方登录，成功后自动登记账号。已有账号的重新授权从账号行发起。</p></div><Link className="button secondary" to="/accounts">账号管理</Link></header>
  {notice?<p role="status">{notice}</p>:null}
  {channels.isPending?<p>读取可用授权方式…</p>:channels.isError?<p role="alert">{asAppError(channels.error).message}</p>:<div className="account-channel-grid" aria-label="官方授权渠道">{channels.data.filter(row=>row.authorization_available&&["codex","claude","kimi-coding","kiro","grok.build"].includes(row.id)).map(row=><div className="surface" key={row.id}><h3>{row.name}</h3><p>{row.authorization_flow==="device"?"设备验证码授权":"官方登录授权"}</p>{row.id==="kimi-coding"&&kimi.length>1?<label>接入服务<select value={provider} onChange={event=>setProvider(event.target.value)}><option value="">请选择服务</option>{kimi.map(target=><option key={target.id} value={target.id}>{target.name} · {target.id}</option>)}</select></label>:null}<button className="primary" disabled={row.id==="kimi-coding"&&(targets.isPending||targets.isError||(kimi.length>1&&!kimi.some(target=>target.id===provider)))} onClick={()=>setChannel(row.id)}>授权 {row.name}</button></div>)}</div>}
  <p className="muted">授权文件在账号管理导入；API Key 在 AI 提供商配置。此页不会检查模型、续期或发起推理。</p>
  {targets.isError?<p role="alert">接入服务读取失败：{asAppError(targets.error).message}</p>:null}
  {channel==="codex"||channel==="claude"?<AuthorizationCodeDialog channel={channel} onClose={()=>setChannel(undefined)} onComplete={complete}/>:null}
  {channel==="kimi-coding"?<KimiDeviceDialog providerId={kimi.length>1?provider:undefined} onClose={()=>setChannel(undefined)} onComplete={complete}/>:null}
  {channel==="kiro"?<KiroDeviceDialog onClose={()=>setChannel(undefined)} onComplete={complete}/>:null}
  {channel==="grok.build"?<GrokDeviceWizard name="" onClose={()=>setChannel(undefined)} onComplete={()=>complete("Grok 授权已保存；请核对运行应用与模型权限。")}/>:null}
 </section>;
}
