import {useMutation} from "@tanstack/react-query";
import {asAppError} from "../../api/errors";
import type {ConfigurationTask,ConfigurationTaskReview} from "../config-versions/configurationTask";
import type {ConfigVersionSummary} from "../config-versions/versionStore";
export type AuthorizationReceipt=Readonly<{state:string;session_id:string;channel:string;config_version:string;upstream_id:string;credential_id:string;started_revision:string;revision:string}>;

/** A reviewed saved authorization must be attached before a new publication review. */
export async function continueAuthorizationAfterReview(task:ConfigurationTask,review:ConfigurationTaskReview,target:Readonly<{upstream_id:string;endpoint_id?:string}>,credentialId:string) {
 await task.acceptReviewedDraft(review);
 const account=await task.read<{id:string;upstream_id:string}>("getCredential",{path:{credential_id:credentialId}});
 if(account.id!==credentialId||account.upstream_id!==target.upstream_id)throw new Error("账号目标已变化，请重新核对。");
 if(target.endpoint_id){
  const bindings=await task.read<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:target.endpoint_id}});
  if(!bindings.some(binding=>binding.credential_id===credentialId))await task.mutate("createEndpointCredentialBinding",{path:{endpoint_id:target.endpoint_id},body:{credential_id:credentialId,enabled:true,priority:0,weight:1,concurrency:1}});
 }
 return {...task.version,revision:task.revision()};
}

/** Recovery reads an exact operation receipt; it never polls or exchanges Provider material. */
export function AuthorizationRecovery({task,sessionId,onReceipt}:Readonly<{task:ConfigurationTask;sessionId:string;onReceipt:(receipt:AuthorizationReceipt,continuable:boolean)=>void}>) {
 const read=useMutation({mutationFn:async()=>{
  const receipt=await task.read<AuthorizationReceipt>("getAccountAuthorizationReceipt",{path:{session_id:sessionId}});
  if(receipt.session_id!==sessionId||receipt.config_version!==task.version.id)throw new Error("授权回执不属于当前会话。");
  let continuable=receipt.state==="completed"?await task.acceptAuthorizationReceipt(receipt):false;
  if(receipt.state==="pending"&&receipt.revision===task.revision()){
    const version=await task.read<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:task.version.id}});
    continuable=version.status==="draft"&&version.revision===receipt.revision;
  }
  return {receipt,continuable};
 },onSuccess:({receipt,continuable})=>onReceipt(receipt,continuable)});
 const labels:Record<string,string>={pending:"会话仍待授权，可明确继续检查",in_progress:"原授权正在处理，请稍后再核对",completed:"授权已确认保存；连接与应用需继续核对",unknown:"授权结果待确认，不会重复交换令牌",failed:"授权未保存",denied:"授权被拒绝",expired:"授权已过期",cancelled:"授权已取消"};
 return <section aria-label="授权结果恢复"><p role="status">先读取本次会话回执。不会重复回调或交换令牌。</p><button className="secondary" disabled={read.isPending} onClick={()=>read.mutate()}>核对授权结果</button>{read.data?<p role="status">{labels[read.data.receipt.state]??"结果待确认"}</p>:null}{read.isError?<p role="alert">{asAppError(read.error).message}。结果保持待确认。</p>:null}</section>;
}
