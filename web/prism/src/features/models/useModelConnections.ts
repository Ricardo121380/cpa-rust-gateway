import { useQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import type { ManagementOperationName } from "../../generated/management-client";
import type { ManagedEndpoint } from "../accounts/inventory";
import { useVersionStore } from "../config-versions/versionStore";
import type { CandidateRecord, RouteListItem, RoutingPage } from "./model";

async function pages<T>(operation:ManagementOperationName) {
  const items:T[]=[];let cursor:string|undefined;let revision:string|undefined;
  do {
    const page=await call<RoutingPage<T>>(operation,{query:{limit:100,...(cursor?{cursor}:{})}},{versionScoped:true});
    if(revision&&revision!==page.revision)throw new Error("连接配置已变化，请刷新后重读。");
    revision=page.revision;items.push(...page.items);cursor=page.next_cursor??undefined;
    if(items.length>=10000&&cursor)throw new Error("连接超过本页读取范围，请使用高级资源列表。");
  }while(cursor);
  return {items,revision};
}

/** Complete configuration topology, separate from catalog contents and runtime availability. */
export function useModelConnections() {
  const scope=useVersionStore(s=>s.context?.configVersionId);
  return useQuery({queryKey:["model-connections",scope],enabled:!!scope,retry:false,queryFn:async()=>{
    const [routes,candidates,endpoints]=await Promise.all([pages<RouteListItem>("listRoutes"),pages<CandidateRecord>("listRouteCandidates"),pages<ManagedEndpoint>("listManagedEndpoints")]);
    if(new Set([routes.revision,candidates.revision,endpoints.revision]).size!==1)throw new Error("连接配置已变化，请刷新后重读。");
    return {routes:routes.items,candidates:candidates.items,endpoints:endpoints.items};
  }});
}
