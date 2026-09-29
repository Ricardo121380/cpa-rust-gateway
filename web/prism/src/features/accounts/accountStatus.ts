type StatusInput=Readonly<{enabled:boolean;auth:string;connections?:number;runtime:readonly string[];readFailed?:boolean;acceptingRequests?:boolean|null;draft?:boolean;successfulCall?:Readonly<{model:string;protocol:string}>}>;
/** One actionable display reason, while the raw evidence remains independent and scoped. */
export function primaryAccountStatus(input:StatusInput):Readonly<{label:string;status:string}>{
  if(!input.enabled)return {label:"已停用",status:"disabled"};
  if(input.acceptingRequests===false)return {label:"服务运行配置待恢复",status:"unavailable"};
  if(["reauth_required","unauthorized","expired","revoked"].includes(input.auth))return {label:"需要重新授权",status:"unauthorized"};
  if(input.draft)return {label:"已保存，待应用",status:"draft"};
  if(input.connections===0)return {label:"未连接接口",status:"missing"};
  if(input.readFailed)return {label:"运行观测读取失败",status:"unknown"};
  const priorities=[{value:"unauthorized",label:"调用被拒绝"},{value:"expired",label:"运行授权已过期"},{value:"quota_blocked",label:"额度阻塞"},{value:"circuit_open",label:"部分连接熔断"},{value:"cooling",label:"部分连接冷却"},{value:"recovery_in_flight",label:"本地恢复进行中"}];
  for(const state of priorities){const count=input.runtime.filter(value=>value===state.value).length;if(count)return {label:`${state.label} · ${count}/${input.runtime.length} 个连接`,status:state.value};}
  if(input.runtime.length===0||input.runtime.some(value=>value!=="available"))return {label:"运行状态未观测",status:"unknown"};
  if(input.successfulCall)return {label:`最近调用通过 · ${input.successfulCall.model} / ${input.successfulCall.protocol}`,status:"succeeded"};
  return {label:"已配置，调用未验证",status:"configured"};
}
