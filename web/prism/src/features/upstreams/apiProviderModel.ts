import type {ManagedEndpoint,InventoryPage} from "../accounts/inventory";
import type {ManagementOperationName,ManagementRequest} from "../../generated/management-client";
export type ApiProvider=Readonly<{id:string;name:string;kind:string;enabled:boolean}>;
export type ApiConnection=Readonly<{provider:ApiProvider;endpoint:ManagedEndpoint}>;
const normalizedBase=(value:string)=>new URL(value).href.replace(/\/+$/u,"");
export function matchingApiConnections(providers:readonly ApiProvider[],endpoints:readonly ManagedEndpoint[],input:Readonly<{kind:string;adapter:string;format:string;base:string;path:string}>):ApiConnection[]{
  try { normalizedBase(input.base); } catch { return []; }
  return endpoints.flatMap(endpoint=>{
    const provider=providers.find(row=>row.id===endpoint.upstream_id&&row.kind===input.kind);
    return provider&&endpoint.adapter_id===input.adapter&&endpoint.api_format===input.format&&normalizedBase(endpoint.base_url)===normalizedBase(input.base)&&endpoint.inference_path===input.path&&["http","sse"].includes(endpoint.transport)?[{provider,endpoint}]:[];
  });
}
export function chooseApiConnection(matches:readonly ApiConnection[],endpointId?:string):ApiConnection|undefined{
  if(matches.length===0)return undefined;
  if(endpointId){const selected=matches.find(row=>row.endpoint.id===endpointId);if(!selected)throw new Error("所选连接已变化，请重新核对。");return selected;}
  if(matches.length!==1)throw new Error("有多个匹配连接，请明确选择后保存。");
  return matches[0];
}
export async function readApiConnections(read:<T>(operation:ManagementOperationName,request?:ManagementRequest)=>Promise<T>){
  const providers=await read<ApiProvider[]>("listUpstreams");const endpoints:ManagedEndpoint[]=[];let cursor:string|undefined;let revision:string|undefined;
  do{const page=await read<InventoryPage<ManagedEndpoint>>("listManagedEndpoints",{query:{limit:100,...(cursor?{cursor}:{})}});if(revision!==undefined&&revision!==page.revision)throw new Error("连接配置已变化，请重新读取。");revision=page.revision;endpoints.push(...page.items);cursor=page.next_cursor??undefined;if(cursor&&endpoints.length>=10000)throw new Error("连接范围未完整读取，请先在高级维护中选择。");}while(cursor);
  return {providers,endpoints};
}
