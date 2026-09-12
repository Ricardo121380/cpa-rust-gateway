import type { Page } from "@playwright/test";

/** Enumerate the same synthetic resources that the runtime response fixtures refer to. */
export async function resourceChoices(page:Page, choices:{routes?:string[];upstreams?:string[];endpoints?:string[];accounts?:string[]}) {
  await page.evaluate(async choices=>{
    const path="/src/generated/management-client.ts";
    const {ManagementApi}=await import(path);
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const response=await original.call(this,operation,request);
      if(!response.ok)return response;
      const ids=operation==="listRoutes"?choices.routes:operation==="listUpstreams"?choices.upstreams:operation==="listManagedEndpoints"?choices.endpoints:operation==="listProviderAccountPools"?choices.accounts:undefined;
      if(!ids)return response;
      const data=await response.json();const items=Array.isArray(data)?data:data.items;
      for(const id of ids){
        if(items.some((row:{id?:string;account_id?:string})=>row.id===id||row.account_id===id))continue;
        if(operation==="listRoutes")items.push({id,public_model_id:"pm-runtime-fixture",policy:"smooth_weighted_round_robin",max_attempts:1,bootstrap_timeout_ms:1000});
        else if(operation==="listUpstreams")items.push({id,name:id,kind:"openai-compatible",enabled:true,tags:[],egress_policy_id:null});
        else if(operation==="listManagedEndpoints")items.push({id,upstream_id:choices.upstreams?.[0]??"relay-a",adapter_id:"openai-compatible.responses",api_format:"openai/responses",base_url:"https://relay.example.test",inference_path:"/responses",models_path:null,transport:"https",enabled:true});
        else items.push({...items[0],account_id:id,presentation:{identity:{email:"member@example.test",phone:null,username:null},provider:"Codex",category:"codex",api_format:"openai/responses",host:"chatgpt.com",source:null}});
      }
      return new Response(JSON.stringify(data),{status:response.status,headers:response.headers});
    };
  },choices);
}
