import {useMutation} from "@tanstack/react-query";
import {useRef,useState} from "react";
import {Sheet} from "../../components/Sheet";
import {asAppError} from "../../api/errors";
import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {safeExternalUrl} from "../upstreams/model";
type Session={state:string;session_id:string;user_code:string;verification_uri:string;expires_at_ms:number;interval_ms:number;credential_id?:string};
export function KiroDeviceDialog({providerId,endpointId,onClose,onComplete,credentialId}:{providerId:string;endpointId:string;credentialId?:string;onClose:()=>void;onComplete:(notice?:string)=>void}) {
 const current=useRef<{task:Awaited<ReturnType<typeof beginConfigurationTask>>;id:string}|undefined>(undefined);
 const [session,setSession]=useState<Session>();const [region,setRegion]=useState("us-east-1");const [startUrl,setStartUrl]=useState("https://view.awsapps.com/start");
 const [workingId,setWorkingId]=useState<string>();const [completed,setCompleted]=useState<ConfigVersionSummary>();const [notice,setNotice]=useState<string>();
 const start=useMutation({mutationFn:async()=>{const task=await beginConfigurationTask("授权 Kiro 账号");const id=credentialId??`kiro-${crypto.randomUUID()}`;current.current={task,id};setWorkingId(task.version.id);return task.read<Session>("startKiroEnrollment",{path:{upstream_id:providerId},headers:{"If-Match":task.version.revision},body:{id,region,start_url:startUrl,replace_existing:!!credentialId}});},onSuccess:setSession});
 const poll=useMutation({mutationFn:async()=>{const entry=current.current;if(!entry||!session)throw new Error("请先开始授权");const {task,id}=entry;const result=await task.mutate<Session>("pollKiroEnrollment",{path:{upstream_id:providerId},body:{id,session_id:session.session_id,replace_existing:!!credentialId}});if(result.state==="pending"){setNotice(`等待官方授权，请至少 ${Math.ceil(result.interval_ms/1000)} 秒后再检查。`);return;}
  if(result.state!=="completed"||!result.credential_id)throw new Error("未收到有效授权完成结果");
  if(endpointId){const bindings=await task.read<{credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:endpointId}});if(!bindings.some(v=>v.credential_id===result.credential_id))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:endpointId},body:{credential_id:result.credential_id,enabled:true,priority:0,weight:1,concurrency:1}});}
  setCompleted(await task.finish());
 }});
 const cancel=useMutation({mutationFn:async()=>{const entry=current.current;if(entry&&session)await entry.task.read("cancelKiroEnrollment",{path:{upstream_id:providerId},headers:{"If-Match":entry.task.version.revision},body:{id:entry.id,session_id:session.session_id,replace_existing:!!credentialId}});},onSuccess:onClose});
 const busy=start.isPending||poll.isPending||cancel.isPending;
 const close=()=>{if(busy)return;if(completed){useVersionStore.getState().select(completed);onComplete("Kiro 账号授权已保存。");}else if(poll.isError)onClose();else cancel.mutate();};
 const url=safeExternalUrl(session?.verification_uri);
 return <Sheet title={credentialId?"重新授权 Kiro 账号":"授权 Kiro 账号"} onEscape={close}>{completed?<p role="status">账号已保存{endpointId?"并连接接口":""}。</p>:session?<><p>在 AWS 官方页面完成登录并输入验证码。</p><strong className="mono">{session.user_code}</strong>{url?<p><a href={url} target="_blank" rel="noopener noreferrer">打开 AWS 授权页</a></p>:null}<p>有效期至 {new Date(session.expires_at_ms).toLocaleTimeString()}</p><button disabled={busy||poll.isError} onClick={()=>poll.mutate()}>我已授权，检查结果</button>{notice?<p role="status">{notice}</p>:null}</>:<div className="sheet-form"><p>AWS Builder ID 使用默认入口；组织账号填写 Identity Center 的地区与启动地址。</p><label>授权地区<input value={region} onChange={e=>setRegion(e.target.value)} disabled={busy}/></label><label>启动地址<input type="url" value={startUrl} onChange={e=>setStartUrl(e.target.value)} disabled={busy}/></label><button disabled={busy||start.isError} onClick={()=>start.mutate()}>开始授权</button></div>}
 <ConfigurationTaskNotice workingId={workingId} error={start.error??poll.error} onReview={version=>{useVersionStore.getState().select(version);onComplete("请核对 Kiro 授权修改。");}}/>{cancel.isError?<p role="alert">{asAppError(cancel.error).message}</p>:null}<div className="sheet-actions"><button className="secondary" disabled={busy} onClick={close}>{completed?"完成":poll.isError?"关闭":"取消"}</button></div></Sheet>;
}
