import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { exactShare, formatPercent, formatMicrounits, type BillingResponse } from "../monitoring/model";
import { costSources, supportsCostFilters } from "./costSources";
import type { Filters } from "./model";

export function UsageCosts({range,filters}:Readonly<{range:Readonly<{from_ms?:number;to_ms?:number}>;filters:Filters}>) {
  const requestSupported=!filters.provider_id&&!filters.channel_id&&!filters.account_id&&!filters.access_group_id&&!filters.protocol;
  const requestQuery={...range,...(filters.model?{model:filters.model}:{}),...(filters.client_key_id?{client_key_id:filters.client_key_id}:{}),limit:1};
  const requests=useQuery({queryKey:["usage-request-summary",requestQuery],enabled:requestSupported,retry:false,
    queryFn:()=>call<{summary:{requests:number;success_rate:number|null;p95_duration_ms:number|null;failed:number;cancelled:number}|null}>("summarizeRequests",{query:requestQuery})});
  const requestSummary=requestSupported&&!requests.isError?requests.data?.summary:undefined;
  const requestLink=new URLSearchParams({tab:"requests"});
  for(const [key,value] of Object.entries(requestQuery))if(value!==undefined&&key!=="limit")requestLink.set(key,String(value));
  const requestNote=!requestSupported?"请求统计暂不支持当前筛选维度":requests.isError?"请求统计读取失败":requests.isPending?"读取中…":"上游重试不重复计数";
  const supported=supportsCostFilters(filters);
  const query={...range,...filters,limit:100};
  const billing=useInfiniteQuery({queryKey:["usage-cost-sources",query],enabled:supported,retry:false,
    initialPageParam:undefined as string|undefined,
    queryFn:({pageParam})=>call<BillingResponse>("listOperationalBilling",{query:{...query,...(pageParam?{cursor:pageParam}:{})}}),
    getNextPageParam:page=>page.next_cursor??undefined});
  const sources=costSources(billing.data?.pages.flatMap(page=>page.items)??[]);
  const summary=!supported||billing.isError?undefined:billing.data?.pages[0]?.summary;
  const ledger=new URLSearchParams({tab:"ledger"});
  for(const [key,value] of Object.entries({...range,...filters}))if(value!==undefined)ledger.set(key,String(value));
  return <>
    <div className="request-metrics overview-kpis usage-headline-metrics">
      <div><span>请求数</span><strong>{requestSummary?.requests.toLocaleString()??"—"}</strong><small>{requestNote}</small></div>
      <div><span>请求成功率</span><strong>{formatPercent(requestSummary?.success_rate??null)}</strong><small>{requestSummary?`${requestSummary.failed} 失败 · ${requestSummary.cancelled} 取消`:requestNote}</small></div>
      <div><span>P95 总耗时</span><strong>{requestSummary?.p95_duration_ms==null?"—":`${Math.round(requestSummary.p95_duration_ms).toLocaleString()} ms`}</strong><small>仅统计已观测总耗时</small></div>
      <div><span>已知费用 · 微单位</span><strong>{summary?.records?formatMicrounits(summary.known_cost_microunits):"—"}</strong><small>{summary?`${summary.unpriced_records} 条记录未计价`:supported?"计价状态待确认":"账本不支持当前筛选维度"}</small></div>
    </div>
    <div className="usage-cost-workspace">
    <section className="usage-cost-panel" aria-label="费用来源"><header><div><h3>费用来源</h3><p>按原始模型归类 · 已知费用</p></div><span className="badge">运营者计价</span></header>
      {!supported?<p className="empty-state">账本暂不支持密钥、访问组或协议筛选。请清除这些条件后核对同范围费用。</p>:billing.isError?<div role="alert"><p>{asAppError(billing.error).message}</p><button className="secondary" onClick={()=>void billing.refetch()}>重新读取费用</button></div>:billing.isPending?<p role="status">读取费用来源…</p>:sources.length?<>
        <table className="usage-cost-table"><thead><tr><th>模型</th><th>已知费用 · 微单位</th><th>计价状态</th></tr></thead><tbody>{sources.map(source=><tr key={source.model}><td><code>{source.model}</code><small>{source.records} 条账本记录</small></td><td>{formatMicrounits(source.known)}</td><td>{source.incomplete?`${source.incomplete} 条未完整计价`:"精确"}</td></tr>)}</tbody></table>
        {billing.hasNextPage?<div className="usage-cost-more"><p>当前仅汇总已载入记录，模型费用尚不完整。</p><button disabled={billing.isFetchingNextPage} onClick={()=>void billing.fetchNextPage()}>加载更多费用记录</button></div>:null}
      </>:<p className="empty-state">当前范围没有账本记录，不能据此判断没有消费。</p>}
      <footer><p>金额使用后台微单位，不擅自指定币种；缺价与未观测金额保留未知。</p>{supported?<Link to={`/monitoring?${ledger}`}>查看同范围账本 →</Link>:null}{requestSupported?<Link className="usage-request-link" to={`/monitoring?${requestLink}`}>查看同范围请求 →</Link>:null}</footer>
    </section>
    <aside className="usage-cost-panel usage-cost-completeness"><h3>费用完整性</h3><p>完整筛选范围 · 独立账本快照</p><strong className="usage-cost-ratio">{formatPercent(summary?exactShare(summary):null)}</strong><p>账本记录的精确计价占比</p><dl>{[
      ["已知费用 · 微单位",summary?.records?formatMicrounits(summary.known_cost_microunits):"—"],
      ["精确计价",summary?`${summary.exact_records} 条`:"—"],
      ["部分计价",summary?`${summary.partial_records} 条`:"—"],
      ["缺少价格",summary?`${summary.unpriced_records} 条`:"—"],
      ["用量未知",summary?`${summary.unknown_records} 条`:"—"],
    ].map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl><p>已知费用不等于全部费用。账本与请求数可能不同，计费处理也可能稍后完成。</p></aside>
  </div></>;
}
