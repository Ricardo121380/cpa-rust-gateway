import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { runtimeStatusMeta, type PoolSnapshot } from "../runtime/model";

/** A complete, bounded runtime snapshot. Never infer absence from a first page. */
export async function readAccountRuntimeSummary(signal?:AbortSignal) {
    const pages:PoolSnapshot[]=[];let cursor:string|undefined;
    do {
      const page=await call<PoolSnapshot>("listProviderAccountPools",{query:{limit:100,...(cursor?{cursor}:{})},signal});
      if(pages.length&&pages[0]!.snapshot_id!==page.snapshot_id)throw new Error("运行快照已变化，请重新读取");
      pages.push(page);cursor=page.next_cursor??undefined;
      if(cursor&&pages.length>=100)throw new Error("运行记录超出本次范围，请在运行状态中筛选查看");
    }while(cursor);
    return {rows:pages.flatMap(page=>page.items),observedAt:pages[0]?.observed_at_ms};
}
export function useAccountRuntimeSummary() {
  return useQuery({queryKey:["account-list-runtime"],retry:false,queryFn:({signal})=>readAccountRuntimeSummary(signal)});
}
export function AccountRuntimeSummary({snapshot,ids,providerId,nativeKind}:Readonly<{
 snapshot:ReturnType<typeof useAccountRuntimeSummary>;ids:readonly string[];providerId?:string;nativeKind?:string;
}>) {
  const rows=snapshot.data?.rows.filter(row=>ids.includes(row.account_id)&&(providerId?row.provider_id===providerId:true)&&(nativeKind?row.account_kind===nativeKind:!row.account_kind.startsWith("grok_")))??[];
  const states=[...new Set(rows.map(row=>row.enabled?runtimeStatusMeta(row.runtime_status).label:"已停用"))];
  return <div className="account-runtime-summary">
    <span>{snapshot.isError?"运行状态读取失败":snapshot.isPending?"读取运行状态…":states.join(" · ")||"未观测运行连接"}</span>
    <small>额度余额未观测</small>
    {snapshot.data&&!snapshot.isError&&rows.length?<small>快照 {new Date(snapshot.data.observedAt!).toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})} · {rows.length} 个连接</small>:null}
    <Link to="/accounts?view=runtime">查看运行证据</Link>
  </div>;
}
