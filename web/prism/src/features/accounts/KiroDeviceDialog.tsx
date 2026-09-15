import {useMutation} from "@tanstack/react-query";
import {useEffect,useRef,useState} from "react";
import {Sheet} from "../../components/Sheet";
import {asAppError} from "../../api/errors";
import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {safeExternalUrl} from "../upstreams/model";
type Session=Readonly<{state:"pending"|"completed"|"cancelled"|"denied"|"expired"|"failed";session_id?:string;user_code?:string;verification_uri?:string;expires_at_ms?:number;interval_ms?:number;credential_id?:string}>;

/** The server may return only state and interval while a device challenge is pending. */
export function mergeKiroDeviceSession(current:Session|undefined,next:Session):Session { return {...current,...next}; }

type PreparedTarget=Readonly<{upstream_id:string;endpoint_id:string}>;

/** Existing reauthorization has an exact owner even when its bindings are intentionally hidden. */
export function existingKiroTarget(providerId:string|undefined,endpointId:string|undefined):PreparedTarget|undefined {
 return providerId?{upstream_id:providerId,endpoint_id:endpointId??""}:undefined;
}

/** Existing canonical Kiro accounts retain their region unless advanced options are explicit. */
export function kiroStartOptions(credentialId:string|undefined,advanced:boolean,region:string,startUrl:string) {
 return !credentialId||advanced?{region,start_url:startUrl}:{};
}

export function KiroDeviceDialog({providerId,endpointId,onClose,onComplete,credentialId}:{providerId?:string;endpointId?:string;credentialId?:string;onClose:()=>void;onComplete:(notice?:string)=>void}) {
 const current=useRef<{task:Awaited<ReturnType<typeof beginConfigurationTask>>;id:string;providerId:string;endpointId:string}|undefined>(undefined);
 const pollingGeneration=useRef(0);
 const [session,setSession]=useState<Session>();const [region,setRegion]=useState("us-east-1");const [startUrl,setStartUrl]=useState("https://view.awsapps.com/start");
 const [advanced,setAdvanced]=useState(false);
 const [workingId,setWorkingId]=useState<string>();const [completed,setCompleted]=useState<ConfigVersionSummary>();const [notice,setNotice]=useState<string>();
 const mergeSession=(next:Session)=>setSession(current=>mergeKiroDeviceSession(current,next));
 const start=useMutation({mutationFn:async()=>{const task=await beginConfigurationTask("授权 Kiro 账号");const id=credentialId??`kiro-${crypto.randomUUID()}`;
  // A reauthorization owns one existing credential/upstream. Its bindings are retained by the
  // server-side CAS replacement and must never trigger a fresh default-region target or binding.
  const target=existingKiroTarget(providerId,endpointId)??await task.mutate<PreparedTarget>("prepareKiroAccountTarget",{body:{region}});
  current.current={task,id,providerId:target.upstream_id,endpointId:target.endpoint_id};setWorkingId(task.version.id);const advancedInput=kiroStartOptions(credentialId,advanced,region,startUrl);return task.read<Session>("startKiroEnrollment",{path:{upstream_id:target.upstream_id},headers:{"If-Match":task.revision()},body:{id,...advancedInput,replace_existing:!!credentialId}});},onSuccess:mergeSession});
 const poll=useMutation({mutationFn:async()=>{const entry=current.current;if(!entry||!session?.session_id)throw new Error("请先开始授权");const {task,id}=entry;const result=await task.mutate<Session>("pollKiroEnrollment",{path:{upstream_id:entry.providerId},body:{id,session_id:session.session_id,replace_existing:!!credentialId}});if(result.state!=="completed"||!result.credential_id){if(result.state==="pending")setNotice(`正在等待官方授权，约 ${Math.ceil((result.interval_ms??5_000)/1000)} 秒后会自动再次检查。`);return result;}
  if(entry.endpointId){const bindings=await task.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:entry.endpointId}});if(!bindings.some(v=>v.credential_id===result.credential_id))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:entry.endpointId},body:{credential_id:result.credential_id,enabled:true,priority:0,weight:1,concurrency:1}});}
  setCompleted(await task.finish());
  return result;
 },onSuccess:mergeSession});
 const cancel=useMutation({mutationFn:async()=>{const entry=current.current;if(entry&&session?.session_id)await entry.task.read("cancelKiroEnrollment",{path:{upstream_id:entry.providerId},headers:{"If-Match":entry.task.revision()},body:{id:entry.id,session_id:session.session_id,replace_existing:!!credentialId}});},onSuccess:onClose});
 const busy=start.isPending||poll.isPending||cancel.isPending;
 const pending=session?.state==="pending"&&!poll.isError&&!poll.isPending;
 useEffect(()=>{if(!pending||!session?.session_id||completed)return;const generation=++pollingGeneration.current;const timer=window.setTimeout(()=>{if(generation===pollingGeneration.current)poll.mutate();},Math.max(1_000,session.interval_ms??5_000));return()=>{pollingGeneration.current+=1;window.clearTimeout(timer);};},[completed,pending,poll.mutate,session?.interval_ms,session?.session_id]);
 const close=()=>{pollingGeneration.current+=1;if(busy)return;if(completed){useVersionStore.getState().select(completed);onComplete("Kiro 账号授权已保存。");}else if(poll.isError)onClose();else if(pending)cancel.mutate();else onClose();};
 const url=safeExternalUrl(session?.verification_uri);
 return <Sheet title={credentialId?"重新授权 Kiro 账号":"授权 Kiro 账号"} onEscape={close}>{completed?<p role="status">账号已保存{current.current?.endpointId?"并连接接口":""}。</p>:session?<><p role="status">{session.state==="pending"?"等待 AWS 官方授权":session.state==="denied"?"授权被拒绝":session.state==="expired"?"授权已过期":"授权未完成"}</p>{pending?<><p>在 AWS 官方页面完成登录并输入验证码。</p><strong className="mono">{session.user_code}</strong>{url?<p><a href={url} target="_blank" rel="noopener noreferrer">打开 AWS 授权页</a></p>:null}{session.expires_at_ms?<p>有效期至 {new Date(session.expires_at_ms).toLocaleTimeString()}。完成后会自动检查结果。</p>:null}</>:null}{notice?<p role="status">{notice}</p>:null}</>:<div className="sheet-form"><p>AWS Builder ID 使用默认入口。组织 Identity Center 设置仅在需要时填写。</p><details open={advanced} onToggle={event=>setAdvanced((event.currentTarget as HTMLDetailsElement).open)}><summary>组织账号高级选项</summary><label>授权地区<input value={region} onChange={e=>setRegion(e.target.value)} disabled={busy}/></label><label>启动地址<input type="url" value={startUrl} onChange={e=>setStartUrl(e.target.value)} disabled={busy}/></label></details><button disabled={busy||start.isError} onClick={()=>start.mutate()}>开始授权</button></div>}
 <ConfigurationTaskNotice workingId={workingId} error={start.error??poll.error} onReview={version=>{useVersionStore.getState().select(version);onComplete("请核对 Kiro 授权修改。");}}/>{cancel.isError?<p role="alert">{asAppError(cancel.error).message}</p>:null}<div className="sheet-actions"><button className="secondary" disabled={busy} onClick={close}>{completed?"完成":poll.isError?"关闭":"取消"}</button></div></Sheet>;
}
