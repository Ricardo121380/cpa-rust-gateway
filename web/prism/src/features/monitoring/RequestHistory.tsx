import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useState, useEffect, useId, type FormEvent } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError, shouldRetryManagementRead } from "../../api/errors";
import { useOverviewBilling } from "../overview/useOverviewBilling";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { StatusBadge } from "../../components/StatusBadge";
import { protocolName } from "../accounts/presentation";
import { ResourcePicker } from "../../components/ResourcePicker";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { costConfidenceLabel, formatMicrounits, errorCodeLabel, type AttemptRow } from "./model";
import { downloadText } from "./export";
import "./requests.css";

export type RequestRow = Readonly<{
  request_id:string;client_key_id:string;model:string;requested_model:string;protocol:string;streaming:boolean;
  upstream_id:string|null;endpoint_id:string|null;credential_id:string|null;attempt_count:number;
  outcome:"succeeded"|"failed"|"cancelled"|"unknown";started_at_ms:number|null;finished_at_ms:number|null;
  duration_ms:number|null;first_content_ms:number|null;error_code:string|null;
  usage:Readonly<Record<string,number|null>>|null;cost_microunits:number|null;cost_confidence:"exact"|"partial"|"unknown"|"unpriced"|null;ledger_records:number;
}>;
type Summary=Readonly<{requests:number;succeeded:number;failed:number;cancelled:number;unknown:number;attempts:number;success_rate:number|null;average_duration_ms:number|null;average_first_content_ms:number|null;p50_duration_ms:number|null;p95_duration_ms:number|null}>;
type Bucket=Readonly<{at_ms:number;requests:number;succeeded:number;failed:number;cancelled:number;average_duration_ms:number|null;average_first_content_ms:number|null}>;
type Page=Readonly<{snapshot:number;ledger_snapshot:number;items:readonly RequestRow[];next_cursor:string|null;summary:Summary|null;series:readonly Bucket[]}>;
const outcomeLabel={succeeded:"成功",failed:"失败",cancelled:"已取消",unknown:"未观测终态"};
const milliseconds=(value:number|null|undefined)=>value==null?"—":`${Math.round(value).toLocaleString()} ms`;
const time=(value:number|null)=>value===null?"时间未观测":new Date(value).toLocaleString();
const FILTERS=["model","upstream_id","credential_id","client_key_id","outcome"] as const;
const RANGE_PRESETS = [24, 168, 720] as const;
type RangePreset = (typeof RANGE_PRESETS)[number] | "custom";
function queryFilters(params:URLSearchParams) {return Object.fromEntries(FILTERS.flatMap(key=>params.get(key)?[[key,params.get(key)!]]:[]));}
function presetHours(value: string | null): (typeof RANGE_PRESETS)[number] {
  const parsed = Number(value);
  return RANGE_PRESETS.includes(parsed as (typeof RANGE_PRESETS)[number])
    ? parsed as (typeof RANGE_PRESETS)[number]
    : 24;
}
export function selectedRequestPreset(params: URLSearchParams): RangePreset {
  const from = Number(params.get("from_ms"));
  const to = Number(params.get("to_ms"));
  // Bounds name an immutable past observation, even when their duration is
  // exactly 24 hours or 7 days. They must not masquerade as a moving preset.
  if (params.has("from_ms") && params.has("to_ms") && Number.isFinite(from) && Number.isFinite(to) && to >= from) return "custom";
  return presetHours(params.get("hours"));
}
export function requestRange(params:URLSearchParams,anchor:number,hours=presetHours(params.get("hours"))) {
  const from=Number(params.get("from_ms")),to=Number(params.get("to_ms"));
  const explicit=params.has("from_ms")&&params.has("to_ms")&&Number.isFinite(from)&&Number.isFinite(to)&&from>=0&&to>=from;
  const from_ms=explicit?from:anchor-hours*3_600_000;
  const to_ms=explicit?to:anchor;
  return {from_ms,to_ms,bucket_ms:to_ms-from_ms>24*3_600_000?86_400_000:3_600_000};
}

/**
 * A request-summary card can deep-link an exact observed range. Applying an
 * additional filter must not quietly turn that link back into "last 24h".
 * Choosing a different range preset is an intentional replacement, so it is
 * the one case that drops the explicit endpoints.
 */
export function requestSearchAfterFilter(
  params: URLSearchParams,
  entries: Iterable<readonly [string, FormDataEntryValue]>,
): URLSearchParams {
  const currentPreset = selectedRequestPreset(params);
  const next = new URLSearchParams({ tab: "requests" });
  let requestedHours = String(currentPreset);
  for (const [key, value] of entries) {
    const text = String(value);
    if (!text) continue;
    next.set(key, text);
    if (key === "hours") requestedHours = text;
  }

  const from = params.get("from_ms");
  const to = params.get("to_ms");
  const exactRange = from !== null && to !== null && Number.isFinite(Number(from)) && Number.isFinite(Number(to));
  if (exactRange && from !== null && to !== null && requestedHours === String(currentPreset)) {
    next.set("from_ms", from);
    next.set("to_ms", to);
  }
  return next;
}

function Trend({series:observed,from,to,bucket,summary,requestLink}:Readonly<{series:readonly Bucket[];from:number;to:number;bucket:number;summary:Summary;requestLink:string}>) {
  const start=Math.floor(from/bucket)*bucket;
  const byTime=new Map(observed.map(row=>[row.at_ms,row]));
  const series=observed.length?Array.from({length:Math.min(1000,Math.floor((to-start)/bucket)+1)},(_,index)=>{const at=start+index*bucket;return byTime.get(at)??{at_ms:at,requests:0,succeeded:0,failed:0,cancelled:0,average_duration_ms:null,average_first_content_ms:null};}):[];
  const [metric,setMetric]=useState<"requests"|"average_duration_ms">("requests");
  const gradient=useId();
  const maximum=Math.max(1,...series.map(row=>row[metric]??0));
  const x=(row:Bucket)=>42+(row.at_ms-start)/Math.max(1,to-start)*690;
  const y=(row:Bucket)=>155-(row[metric]??0)/maximum*115;
  // Unknown latency buckets break the line instead of inventing zero latency.
  const segments:Bucket[][]=[];
  let current:Bucket[]=[];
  for(const row of series){if(row[metric]===null){if(current.length)segments.push(current);current=[];}else current.push(row);}
  if(current.length)segments.push(current);
  return <figure className="request-trend"><figcaption><strong>请求趋势</strong><div className="trend-modes" role="group" aria-label="趋势指标"><button type="button" aria-pressed={metric==="requests"} onClick={()=>setMetric("requests")}>请求量</button><button type="button" aria-pressed={metric==="average_duration_ms"} onClick={()=>setMetric("average_duration_ms")}>平均耗时</button></div></figcaption>
    <p className="trend-summary"><strong>{metric==="requests"?summary.requests.toLocaleString():milliseconds(summary.average_duration_ms)}</strong><span>{metric==="requests"?"次请求 · 当前时间范围":"平均耗时 · 已观测请求"}</span></p>
    {series.length?<svg viewBox="0 0 760 185" role="img" aria-label={metric==="requests"?"每个时间段的实际请求数":"每个时间段已观测的平均耗时"}>
      <defs><linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1"><stop offset="0%" className="trend-fill-start"/><stop offset="100%" className="trend-fill-end"/></linearGradient></defs>
      {[0,.5,1].map(part=><g key={part}><line x1="42" y1={155-part*115} x2="732" y2={155-part*115} className="request-axis"/><text x="34" y={159-part*115} textAnchor="end" className="trend-axis-label">{Math.round(maximum*part).toLocaleString()}</text></g>)}
      {segments.map((segment,index)=>{const points=segment.map(row=>`${x(row)},${y(row)}`).join(" ");return <g key={index}>{metric==="requests"?<polygon points={`${x(segment[0]!)},155 ${points} ${x(segment[segment.length-1]!)},155`} fill={`url(#${gradient})`}/>:null}<polyline points={points} className="request-line"/>{segment.map(row=><circle key={row.at_ms} cx={x(row)} cy={y(row)} r="2.5"><title>{new Date(row.at_ms).toLocaleString()}：{metric==="requests"?`${row.requests} 次，成功 ${row.succeeded}，失败 ${row.failed}，取消 ${row.cancelled}`:milliseconds(row.average_duration_ms)}</title></circle>)}</g>;})}
      <text x="42" y="179" className="trend-axis-label">{new Date(from).toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})}</text><text x="732" y="179" textAnchor="end" className="trend-axis-label">{new Date(to).toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})}{metric==="average_duration_ms"?" · ms":""}</text>
    </svg>:<p className="empty-state">此时间范围没有已观测的请求终态。</p>}
    <div className="trend-footer"><Link to={requestLink}>查看此范围请求 →</Link><details><summary>趋势数据与时间范围</summary><p className="trend-range">{new Date(from).toLocaleString()} — {new Date(to).toLocaleString()}</p><div className="tablewrap"><table><thead><tr><th>时间</th><th>请求</th><th>成功</th><th>失败</th><th>取消</th><th>平均耗时</th></tr></thead><tbody>{series.map(row=><tr key={row.at_ms}><td>{time(row.at_ms)}</td><td>{row.requests}</td><td>{row.succeeded}</td><td>{row.failed}</td><td>{row.cancelled}</td><td>{milliseconds(row.average_duration_ms)}</td></tr>)}</tbody></table></div></details></div>
  </figure>;
}
function Metrics({data,link}:Readonly<{data:Summary;link:string}>) {
  return <div className="request-metrics">{[
    ["请求数",data.requests.toLocaleString(),""],
    ["成功率",data.success_rate===null?"—":`${(data.success_rate*100).toFixed(1)}%`,"succeeded"],
    ["失败请求",data.failed.toLocaleString(),"failed"],
    ["P50 / P95",`${milliseconds(data.p50_duration_ms)} / ${milliseconds(data.p95_duration_ms)}`,""],
    ["平均首内容延迟",milliseconds(data.average_first_content_ms),""],
  ].map(([label,value,outcome])=><Link key={label} to={(()=>{const [path,query]=link.split("?");const params=new URLSearchParams(query);if(outcome)params.set("outcome",outcome);return `${path}?${params}`;})()}><span title={label==="平均首内容延迟"?"仅统计已观测到内容的请求，分母与总耗时不同。":label==="请求数"?"按已受理的推理请求计数，上游重试不重复计数。":undefined}>{label}</span><strong>{value}</strong></Link>)}</div>;
}
export function RequestOverview({onRangeChange, title}:Readonly<{onRangeChange?:(range:{from_ms:number;to_ms:number})=>void;title?:string}>) {
  const [hours,setHours]=useState(24),[anchor,setAnchor]=useState(Date.now());
  const range={from_ms:anchor-hours*3_600_000,to_ms:anchor,bucket_ms:hours>24?86_400_000:3_600_000};
  const billingRange={from_ms:range.from_ms,to_ms:range.to_ms};
  useEffect(()=>{onRangeChange?.({from_ms:range.from_ms,to_ms:range.to_ms});},[onRangeChange,range.from_ms,range.to_ms]);
  const query=useQuery({queryKey:["request-summary",range],queryFn:()=>call<Page>("summarizeRequests",{query:{...range,limit:1}}),retry:shouldRetryManagementRead,retryDelay:(attempt)=>250*(attempt+1)});
  const billing=useOverviewBilling(billingRange);
  const link=`/monitoring?tab=requests&from_ms=${range.from_ms}&to_ms=${range.to_ms}`;
  const ledger=`/monitoring?tab=ledger&from_ms=${range.from_ms}&to_ms=${range.to_ms}`;
  const summary=query.data?.summary;
  const costs=billing.isError?undefined:billing.data?.summary;
  return <section className="request-overview">
    <header className="page-head"><div><p className="overview-eyebrow">YOUR GATEWAY, AT A GLANCE</p><h2>{title ? "运行概览" : "请求概览"}</h2><p className="page-description">请求、账号与费用，汇集于同一工作台。</p></div><div className="overview-range-context"><div className="overview-range" role="group" aria-label="请求时间范围">{([[24,"24 小时"],[168,"7 天"],[720,"30 天"]] as const).map(([value,label])=><button key={value} type="button" aria-pressed={hours===value} onClick={()=>{setHours(value);setAnchor(Date.now());}}>{label}</button>)}<button type="button" aria-label="刷新请求概览" onClick={()=>setAnchor(Date.now())}>刷新</button></div><p className="overview-range-note">截至 {new Date(anchor).toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})}</p></div></header>
    {query.isError?<p role="alert">{asAppError(query.error).message}</p>:summary?<>
      <div className="request-metrics overview-kpis">
        <Link to={link}><span>请求数</span><strong>{summary.requests.toLocaleString()}</strong><small>上游重试不重复计数</small></Link>
        <Link to={`${link}&outcome=succeeded`}><span>成功率</span><strong>{summary.success_rate===null?"—":<>{(summary.success_rate*100).toFixed(1)}<small className="metric-unit">%</small></>}</strong><small>{summary.failed} 失败 · {summary.cancelled} 取消</small></Link>
        <Link to={link}><span>P95 总耗时</span><strong>{summary.p95_duration_ms===null?"—":<>{Math.round(summary.p95_duration_ms).toLocaleString()}<small className="metric-unit">ms</small></>}</strong><small>仅统计已观测总耗时</small></Link>
        <Link to={ledger}><span>已知费用 · 微单位</span><strong>{costs?.records?formatMicrounits(costs.known_cost_microunits):"—"}</strong><small>{billing.isError?"费用暂时无法读取":costs===undefined?"读取中…":costs.records===0?"尚无账本记录":`${costs.unpriced_records} 条记录未计价`}</small></Link>
      </div>
      <div className="overview-observations">
        <Trend series={query.data?.series ?? []} from={range.from_ms} to={range.to_ms} bucket={range.bucket_ms} summary={summary} requestLink={link}/>
        <aside className="overview-attention"><h3>需要关注</h3><p className="stat-sub">当前时间范围内的处理事项</p>
          <Link to={`${link}&outcome=failed`}><span className="attention-symbol" data-tone={summary.failed>0?"warning":"neutral"} aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M12 3 2 21h20L12 3Z M12 9v5 M12 17v1"/></svg></span><div><strong>{summary.failed>0?`${summary.failed} 个失败请求`:"未发现失败请求"}</strong><p>查看错误与上游重试链</p></div><span className="attention-chevron" aria-hidden="true">›</span></Link>
          <Link to={ledger}><span className="attention-symbol" data-tone="info" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M5 18v-5 M12 18V5 M19 18V9"/></svg></span><div><strong>{costs===undefined?"计价状态待确认":costs.records===0?"尚无账本记录":`${costs.unpriced_records} 条记录尚未计价`}</strong><p>已知费用不等于全部费用</p></div><span className="attention-chevron" aria-hidden="true">›</span></Link>
          <Link to="/accounts?view=runtime"><span className="attention-symbol" data-tone="neutral" aria-hidden="true"><svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2"/></svg></span><div><strong>账号健康与额度</strong><p>查看当前认证与调度状态</p></div><span className="attention-chevron" aria-hidden="true">›</span></Link>
          <details><summary>更多请求指标</summary><p>P50：{milliseconds(summary.p50_duration_ms)}</p><p>平均首内容延迟：{milliseconds(summary.average_first_content_ms)}</p><p>{summary.attempts} 次上游尝试 · {summary.unknown} 条终态未知</p></details>
        </aside>
      </div>
    </>:<p role="status">读取请求统计…</p>}
  </section>;
}

function RequestDetail({row,onClose}:Readonly<{row:RequestRow;onClose:()=>void}>) {
  const attempts=useQuery({queryKey:["request-attempts",row.request_id],queryFn:()=>call<readonly AttemptRow[]>("listRequestAttempts",{path:{request_id:row.request_id}}),retry:false});
  return <Sheet title="请求详情" layout="inspector" onEscape={onClose} footer={<SheetDismissButton>关闭</SheetDismissButton>}>
    <header className="request-inspector-header"><h3>{row.model}</h3><StatusBadge status={row.outcome==="succeeded"?"active":row.outcome==="failed"?"unauthorized":"disabled"}>{outcomeLabel[row.outcome]}</StatusBadge><p>{protocolName(row.protocol)} · {row.streaming?"流式请求":"非流式请求"}</p></header>
    <section className="request-inspector-section"><h4>时间与计价</h4><dl className="request-facts"><dt>开始</dt><dd>{time(row.started_at_ms)}</dd><dt>结束</dt><dd>{time(row.finished_at_ms)}</dd><dt>总耗时</dt><dd>{milliseconds(row.duration_ms)}</dd><dt>首内容延迟</dt><dd>{milliseconds(row.first_content_ms)}</dd><dt>费用（微单位）</dt><dd>{row.ledger_records?`${row.cost_microunits===null?"—":formatMicrounits(row.cost_microunits)} · ${row.cost_confidence?costConfidenceLabel(row.cost_confidence):""}`:"尚无账本记录"}</dd>{row.error_code?<><dt>错误</dt><dd>{errorCodeLabel(row.error_code)}</dd></>:null}</dl></section>
    {row.usage?<details className="request-inspector-section"><summary>Token 用量</summary><dl className="request-facts">{Object.entries(row.usage).map(([name,value])=><div key={name}><dt>{name}</dt><dd>{value??"未观测"}</dd></div>)}</dl></details>:null}
    <section className="request-inspector-section"><h4>上游尝试 <span>{row.attempt_count} 次</span></h4>{attempts.isError?<p role="alert">{asAppError(attempts.error).message}</p>:attempts.isPending?<p role="status">读取尝试…</p>:!attempts.data.length?<p className="muted">暂无可读取的尝试记录。</p>:<ol className="request-attempts">{attempts.data.map((attempt,index)=><li key={attempt.attempt_id}><strong>尝试 {index+1} · {attempt.outcome==="succeeded"?"已建立上游响应":attempt.outcome}</strong>{attempt.credential_id?<ResourceIdentity id={attempt.credential_id} kind="account"/>:null}{attempt.endpoint_id&&attempt.credential_id?<Link to={`/runtime?${new URLSearchParams({endpoint_id:attempt.endpoint_id,credential_id:attempt.credential_id})}`} onClick={onClose}>查看诊断</Link>:null}</li>)}</ol>}</section>
    <details className="request-reference request-inspector-section"><summary>请求标识</summary><code>{row.request_id}</code></details>
  </Sheet>;
}
export function RequestHistoryPanel() {
  const [params,setParams]=useSearchParams();const [anchor,setAnchor]=useState(Date.now());
  const preset=selectedRequestPreset(params);const hours=typeof preset==="number"?preset:24;const range=requestRange(params,anchor,hours);const filters=queryFilters(params);
  const query={...range,...filters,include_unknown:params.get("include_unknown")==="true",limit:50};
  const requests=useInfiniteQuery({queryKey:["requests",query],initialPageParam:undefined as string|undefined,queryFn:({pageParam})=>call<Page>("summarizeRequests",{query:{...query,...(pageParam?{cursor:pageParam}:{})}}),getNextPageParam:page=>page.next_cursor??undefined,retry:shouldRetryManagementRead,retryDelay:(attempt)=>250*(attempt+1)});
  const summary=requests.data?.pages[0];
  const [detail,setDetail]=useState<RequestRow>();
  const rows=requests.data?.pages.flatMap(page=>page.items)??[];
  function apply(event:FormEvent<HTMLFormElement>) {event.preventDefault();const data=new FormData(event.currentTarget);setAnchor(Date.now());setParams(requestSearchAfterFilter(params,data.entries()));}
  const link=`/monitoring?${new URLSearchParams({...Object.fromEntries(params),tab:"requests",from_ms:String(range.from_ms),to_ms:String(range.to_ms)})}`;
  return <section className="request-history"><form className="request-filters" key={params.toString()} onSubmit={apply}>
    <label>时间<select name="hours" defaultValue={preset}>{preset==="custom"?<option value="custom" disabled>当前链接范围</option>:null}<option value={24}>最近24小时</option><option value={168}>最近7天</option><option value={720}>最近30天</option></select></label>
    <label>模型<input name="model" defaultValue={params.get("model")??""} placeholder="原始模型 ID"/></label>
    <label>结果<select name="outcome" defaultValue={params.get("outcome")??""}><option value="">全部</option>{Object.entries(outcomeLabel).map(([value,name])=><option key={value} value={value}>{name}</option>)}</select></label>
    <details className="request-extra-filters" open={params.has("upstream_id")||params.has("credential_id")||params.has("client_key_id")||params.get("include_unknown")==="true" ? true : undefined}><summary>渠道、账号及密钥筛选</summary><div>    <label>渠道<ResourcePicker kind="upstream" name="upstream_id" defaultValue={params.get("upstream_id")??""}/></label>
    <label>账号<ResourcePicker kind="account" runtime name="credential_id" defaultValue={params.get("credential_id")??""}/></label>
    <label>API 密钥<ResourcePicker kind="key" name="client_key_id" defaultValue={params.get("client_key_id")??""}/></label>

    <label className="check-row"><input type="checkbox" name="include_unknown" value="true" defaultChecked={params.get("include_unknown")==="true"}/>包含时间和终态未知的历史请求</label>
</div></details>
    <div className="request-filter-actions"><button type="submit">应用筛选</button><button type="button" className="secondary" onClick={()=>{setAnchor(Date.now());setParams({tab:"requests"});}}>重置</button></div>
  </form>
  <div className="request-results-toolbar">
    {summary?.summary?<details className="request-summary-disclosure"><summary>{summary.summary.requests.toLocaleString()} 个请求 · {summary.summary.failed.toLocaleString()} 个失败 <span>统计与延迟</span></summary><Metrics data={summary.summary} link={link}/><p className="stat-sub">重试单列：{summary.summary.attempts} 次上游尝试 · {summary.summary.unknown} 条终态未知 · {summary.summary.cancelled} 条已取消</p></details>:<span/>}
    <div className="request-read-actions"><button className="secondary" onClick={()=>{setAnchor(Date.now());void requests.refetch();}}>重新读取</button><button className="secondary" disabled={!rows.length} onClick={()=>downloadText(`requests-${Date.now()}.jsonl`,rows.map(row=>JSON.stringify(row)).join("\n"))}>导出已载入记录（{rows.length}）</button></div>
  </div>
  {requests.isError?<p role="alert">{asAppError(requests.error).message}</p>:requests.isPending?<p role="status">读取请求记录…</p>:!rows.length?<p className="empty-state">此筛选下没有请求记录。</p>:<div className="tablewrap request-records-panel"><table className="request-table"><thead><tr><th>模型 / 时间</th><th>渠道 / 账号</th><th>结果</th><th>耗时 / 首内容</th><th>费用（微单位）</th><th>操作</th></tr></thead><tbody>{rows.map(row=><tr key={row.request_id}><td><strong>{row.model}</strong><small>{time(row.finished_at_ms)}</small></td><td><div>{row.upstream_id?<ResourceIdentity id={row.upstream_id} kind="upstream"/>:"—"}</div>{row.credential_id?<small><ResourceIdentity id={row.credential_id} kind="account"/></small>:null}</td><td><StatusBadge status={row.outcome==="succeeded"?"active":row.outcome==="failed"?"unauthorized":"disabled"}>{outcomeLabel[row.outcome]}</StatusBadge><small>{row.attempt_count} 次尝试</small></td><td>{milliseconds(row.duration_ms)}<small>{milliseconds(row.first_content_ms)}</small></td><td>{row.ledger_records?row.cost_microunits===null?(row.cost_confidence?costConfidenceLabel(row.cost_confidence):"金额未观测"):formatMicrounits(row.cost_microunits):"无账本记录"}</td><td><button className="secondary" onClick={()=>setDetail(row)}>详情</button></td></tr>)}</tbody></table></div>}
  {requests.hasNextPage?<button className="secondary" disabled={requests.isFetchingNextPage||requests.isError} onClick={()=>void requests.fetchNextPage()}>加载更多</button>:null}
  {detail?<RequestDetail row={detail} onClose={()=>setDetail(undefined)}/>:null}
  </section>;
}
