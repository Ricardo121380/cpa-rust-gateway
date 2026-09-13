import {useQuery} from "@tanstack/react-query";
import {call} from "../api/client";
import type {ManagementOperationName} from "../generated/management-client";
import {useVersionStore} from "../features/config-versions/versionStore";
import type {ManagedCredential,ManagedEndpoint} from "../features/accounts/inventory";
import type {NativeAccount} from "../features/accounts/NativeAccounts";
import {accountName,protocolName} from "../features/accounts/presentation";
import {resourceName,type ResourceKind} from "./resourceNames";
async function all<T>(operation:ManagementOperationName,scoped:boolean):Promise<T[]> {
  const rows:T[]=[];let cursor:string|undefined;let revision:string|undefined;
  do {
    const page=await call<{items:T[];next_cursor:string|null;revision?:string}>(operation,{query:{limit:100,...(cursor?{cursor}:{})}},{versionScoped:scoped});
    if(revision&&page.revision!==revision)throw new Error("资源已改变，请重新读取。");
    revision=page.revision;rows.push(...page.items);cursor=page.next_cursor??undefined;
    if(rows.length>=10000&&cursor)throw new Error("资源数量超过本次名称查询范围。");
  }while(cursor);
  return rows;
}
/** Shared query caches resolve real names; IDs stay untouched in links and mutations. */
export function useResourceLabel(id:string,kind:ResourceKind,supplied?:string|null) {
  const context=useVersionStore(state=>state.context);
  const supported=["account","upstream","endpoint","group","key"].includes(kind);
  const query=useQuery({queryKey:["resource-labels",context?.configVersionId,context?.revision,kind],enabled:!supplied&&supported&&!!context,retry:false,staleTime:10_000,queryFn:async()=>{
    const map=new Map<string,string>();
    if(kind==="account") {
      const [managed,native]=await Promise.all([all<ManagedCredential>("listManagedCredentials",true),all<NativeAccount>("listNativeAccounts",false)]);
      for(const row of managed)map.set(row.credential.id,accountName(row.identity)??"未提供账号身份");
      for(const row of native)map.set(row.id,accountName(row.identity)??"未提供账号身份");
    } else if(kind==="endpoint") {
      for(const row of await all<ManagedEndpoint>("listManagedEndpoints",true))map.set(row.id,`${protocolName(row.api_format)} · ${new URL(row.base_url).host}`);
    } else {
      const operation=kind==="upstream"?"listUpstreams":kind==="group"?"listAccessGroups":"listClientKeys";
      const rows=await call<{id:string;name?:string;prefix?:string;access_group_id?:string}[]>(operation,{},{versionScoped:true});
      if(kind==="key") {
        const groups=await call<{id:string;name:string}[]>("listAccessGroups",{},{versionScoped:true});
        const names=new Map(groups.map(group=>[group.id,group.name]));
        for(const row of rows)map.set(row.id,resourceName(row.id,kind,names.get(row.access_group_id??"")));
      } else for(const row of rows)map.set(row.id,resourceName(row.id,kind,row.name));
    }
    return map;
  }});
  if(supplied)return resourceName(id,kind,supplied);
  const name=query.data?.get(id);
  if(name)return name;
  if(supported)return query.isPending&&context?"读取名称…":kind==="account"?"历史账号":kind==="upstream"?"历史提供商":kind==="endpoint"?"历史接口":kind==="key"?"历史密钥":"历史访问组";
  return resourceName(id,kind);
}
