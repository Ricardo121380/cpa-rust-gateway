import {useInfiniteQuery} from "@tanstack/react-query";
import {call} from "../../api/client";
import {useVersionStore} from "../config-versions/versionStore";
import type {ManagedCredential} from "./inventory";
import type {NativeAccount} from "./NativeAccounts";
export type UnifiedAccount=Readonly<{id:string;native:boolean;category:string;provider:string;status:string;plan:string|null;plan_source:string|null;operations:readonly string[];managed:ManagedCredential|null;native_account:NativeAccount|null}>;
type Page=Readonly<{config_version:string;revision:string;total:number;plan_totals:Record<string,number>;unobserved_plan_total:number;category_totals:Record<string,number>;items:readonly UnifiedAccount[];next_cursor:string|null}>;
export function useAccountDirectory(query:Readonly<{q:string;category:string;status:string;sort:string;upstream_id:string;plan?:string;without_plan?:string}>) {
 const context=useVersionStore(s=>s.context);
 return useInfiniteQuery({queryKey:["account-directory",context?.configVersionId,context?.revision,query],enabled:!!context,retry:false,initialPageParam:undefined as string|undefined,
  queryFn:({pageParam})=>call<Page>("listAccountInventory",{query:{limit:50,...Object.fromEntries(Object.entries(query).filter(([,v])=>v!=null&&v!=="")),...(pageParam?{cursor:pageParam}:{})}},{versionScoped:true}),
  getNextPageParam:page=>page.next_cursor??undefined});
}
