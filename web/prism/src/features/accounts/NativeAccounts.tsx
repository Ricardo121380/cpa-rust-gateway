import {useInfiniteQuery} from "@tanstack/react-query";
import {call} from "../../api/client";
import type {AccountIdentity} from "./presentation";
export type NativeAccount = Readonly<{id:string;provider:"grok_build"|"grok_console"|"grok_web";auth_status:string;enabled:boolean;revision:number;import_batch_id:string;identity:AccountIdentity}>;
type Page=Readonly<{items:readonly NativeAccount[];next_cursor:string|null}>;
export function useNativeAccounts() {
  return useInfiniteQuery({queryKey:["native-accounts"],initialPageParam:undefined as string|undefined,retry:false,
    queryFn:({pageParam})=>call<Page>("listNativeAccounts",{query:{limit:100,...(pageParam?{cursor:pageParam}:{})}}),getNextPageParam:(last)=>last.next_cursor??undefined});
}
