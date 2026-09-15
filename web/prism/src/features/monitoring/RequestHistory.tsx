import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useState, useEffect, type FormEvent } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError, shouldRetryManagementRead } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
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
  if (params.get("hours") === "custom") return "custom";
  if (params.has("hours")) return presetHours(params.get("hours"));
  const from = Number(params.get("from_ms"));
  const to = Number(params.get("to_ms"));
  if (!params.has("from_ms") || !params.has("to_ms") || !Number.isFinite(from) || !Number.isFinite(to) || to < from) return 24;
  const span = to - from;
  return RANGE_PRESETS.find((hours) => Math.abs(span - hours * 3_600_000) <= 60_000) ?? "custom";
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

function Trend({series:observed,from,to,bucket}:Readonly<{series:readonly Bucket[];from:number;to:number;bucket:number}>) {
  const start=Math.floor(from/bucket)*bucket;
  const byTime=new Map(observed.map(row=>[row.at_ms,row]));
  const series=observed.length?Array.from({length:Math.min(1000,Math.floor((to-start)/bucket)+1)},(_,index)=>{const at=start+index*bucket;return byTime.get(at)??{at_ms:at,requests:0,succeeded:0,failed:0,cancelled:0,average_duration_ms:null,average_first_content_ms:null};}):[];
  const maximum=Math.max(1,...series.map(row=>row.requests));
  const points=series.map(row=>`${20+(row.at_ms-start)/Math.max(1,to-start)*720},${135-row.requests/maximum*115}`).join(" ");
  return <figure className="request-trend"><figcaption>请求趋势 <span>{new Date(from).toLocaleString()} — {new Date(to).toLocaleString()}</span></figcaption>
    {series.length?<svg viewBox="0 0 760 155" role="img" aria-label="每个时间段的实际请求数"><line x1="20" y1="135" x2="740" y2="135" className="request-axis"/><polyline points={points} className="request-line"/>{series.map(row=><circle key={row.at_ms} cx={20+(row.at_ms-start)/Math.max(1,to-start)*720} cy={135-row.requests/maximum*115} r="3"><title>{new Date(row.at_ms).toLocaleString()}：{row.requests} 次，成功 {row.succeeded}，失败 {row.failed}，取消 {row.cancelled}</title></circle>)}</svg>:<p className="empty-state">此时间范围没有已观测的请求终态。</p>}
    <details><summary>查看趋势数据</summary><div className="tablewrap"><table><thead><tr><th>时间</th><th>请求</th><th>成功</th><th>失败</th><th>取消</th><th>平均耗时</th></tr></thead><tbody>{series.map(row=><tr key={row.at_ms}><td>{time(row.at_ms)}</td><td>{row.requests}</td><td>{row.succeeded}</td><td>{row.failed}</td><td>{row.cancelled}</td><td>{milliseconds(row.average_duration_ms)}</td></tr>)}</tbody></table></div></details>
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
export function RequestOverview({onRangeChange}:Readonly<{onRangeChange?:(range:{from_ms:number;to_ms:number})=>void}>) {
  const [hours,setHours]=useState(24),[anchor,setAnchor]=useState(Date.now());
  const range={from_ms:anchor-hours*3_600_000,to_ms:anchor,bucket_ms:hours>24?86_400_000:3_600_000};
  useEffect(()=>{onRangeChange?.({from_ms:range.from_ms,to_ms:range.to_ms});},[onRangeChange,range.from_ms,range.to_ms]);
  const query=useQuery({queryKey:["request-summary",range],queryFn:()=>call<Page>("summarizeRequests",{query:{...range,limit:1}}),retry:shouldRetryManagementRead,retryDelay:(attempt)=>250*(attempt+1)});
  const link=`/monitoring?tab=requests&from_ms=${range.from_ms}&to_ms=${range.to_ms}`;
  return <section className="request-overview"><div className="data-toolbar"><h3>请求概览</h3><select aria-label="请求时间范围" value={hours} onChange={event=>{setHours(Number(event.target.value));setAnchor(Date.now());}}><option value={24}>最近24小时</option><option value={168}>最近7天</option><option value={720}>最近30天</option></select><button className="secondary" onClick={()=>setAnchor(Date.now())}>刷新</button></div>
    {query.isError?<p role="alert">{asAppError(query.error).message}</p>:query.data?.summary?<><Metrics data={query.data.summary} link={link}/><Trend series={query.data.series} from={range.from_ms} to={range.to_ms} bucket={range.bucket_ms}/><Link to={link}>查看此范围请求</Link></>:<p role="status">读取请求统计…</p>}
  </section>;
}
function RequestDetail({row,onClose}:Readonly<{row:RequestRow;onClose:()=>void}>) {
  const attempts=useQuery({queryKey:["request-attempts",row.request_id],queryFn:()=>call<readonly AttemptRow[]>("listRequestAttempts",{path:{request_id:row.request_id}}),retry:false});
  return <Sheet title={row.model} layout="inspector" onEscape={onClose}>
    <dl className="request-facts"><dt>结果</dt><dd>{outcomeLabel[row.outcome]}</dd><dt>开始</dt><dd>{time(row.started_at_ms)}</dd><dt>结束</dt><dd>{time(row.finished_at_ms)}</dd><dt>总耗时</dt><dd>{milliseconds(row.duration_ms)}</dd><dt>首内容延迟</dt><dd>{milliseconds(row.first_content_ms)}</dd><dt>费用（微单位）</dt><dd>{row.ledger_records?`${row.cost_microunits===null?"—":formatMicrounits(row.cost_microunits)} · ${row.cost_confidence?costConfidenceLabel(row.cost_confidence):""}`:"尚无账本记录"}</dd><dt>上游尝试</dt><dd>{row.attempt_count}</dd><dt>错误</dt><dd>{row.error_code?errorCodeLabel(row.error_code):"—"}</dd></dl>
    {row.usage?<details><summary>Token 用量</summary><dl className="request-facts">{Object.entries(row.usage).map(([name,value])=><div key={name}><dt>{name}</dt><dd>{value??"未观测"}</dd></div>)}</dl></details>:null}
    <h4>重试链</h4>{attempts.isError?<p role="alert">{asAppError(attempts.error).message}</p>:attempts.isPending?<p>读取尝试…</p>:<ol className="request-attempts">{attempts.data.map((attempt,index)=><li key={attempt.attempt_id}><strong>尝试 {index+1} · {attempt.outcome==="succeeded"?"已建立上游响应":attempt.outcome}</strong>{attempt.credential_id?<ResourceIdentity id={attempt.credential_id} kind="account"/>:null}{attempt.endpoint_id&&attempt.credential_id?<Link to={`/runtime?${new URLSearchParams({endpoint_id:attempt.endpoint_id,credential_id:attempt.credential_id})}`} onClick={onClose}>查看诊断</Link>:null}</li>)}</ol>}
    <details className="request-reference"><summary>请求标识</summary><code>{row.request_id}</code></details>
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
    <label>渠道<ResourcePicker kind="upstream" name="upstream_id" defaultValue={params.get("upstream_id")??""}/></label>
    <label>账号<ResourcePicker kind="account" runtime name="credential_id" defaultValue={params.get("credential_id")??""}/></label>
    <label>API 密钥<ResourcePicker kind="key" name="client_key_id" defaultValue={params.get("client_key_id")??""}/></label>
    <label>结果<select name="outcome" defaultValue={params.get("outcome")??""}><option value="">全部</option>{Object.entries(outcomeLabel).map(([value,name])=><option key={value} value={value}>{name}</option>)}</select></label>
    <label className="check-row"><input type="checkbox" name="include_unknown" value="true" defaultChecked={params.get("include_unknown")==="true"}/>包含时间和终态未知的历史请求</label>
    <button type="submit">应用筛选</button><button type="button" className="secondary" onClick={()=>{setAnchor(Date.now());setParams({tab:"requests"});}}>重置</button>
  </form>
  {summary?.summary?<><Metrics data={summary.summary} link={link}/><p className="stat-sub">重试单列：{summary.summary.attempts} 次上游尝试 · {summary.summary.unknown} 条终态未知 · {summary.summary.cancelled} 条已取消</p></>:null}
  <div className="data-toolbar"><button className="secondary" onClick={()=>{setAnchor(Date.now());void requests.refetch();}}>重新读取</button><button className="secondary" disabled={!rows.length} onClick={()=>downloadText(`requests-${Date.now()}.jsonl`,rows.map(row=>JSON.stringify(row)).join("\n"))}>导出已载入记录（{rows.length}）</button></div>
  {requests.isError?<p role="alert">{asAppError(requests.error).message}</p>:requests.isPending?<p role="status">读取请求记录…</p>:!rows.length?<p className="empty-state">此筛选下没有请求记录。</p>:<div className="tablewrap"><table className="request-table"><thead><tr><th>模型 / 时间</th><th>渠道 / 账号</th><th>结果</th><th>耗时 / 首内容</th><th>费用（微单位）</th><th>操作</th></tr></thead><tbody>{rows.map(row=><tr key={row.request_id}><td><strong>{row.model}</strong><small>{time(row.finished_at_ms)}</small></td><td>{row.upstream_id?<ResourceIdentity id={row.upstream_id} kind="upstream"/>:"—"}{row.credential_id?<ResourceIdentity id={row.credential_id} kind="account"/>:null}</td><td>{outcomeLabel[row.outcome]}<small>{row.attempt_count} 次尝试</small></td><td>{milliseconds(row.duration_ms)}<small>{milliseconds(row.first_content_ms)}</small></td><td>{row.ledger_records?row.cost_microunits===null?(row.cost_confidence?costConfidenceLabel(row.cost_confidence):"金额未观测"):formatMicrounits(row.cost_microunits):"无账本记录"}</td><td><button className="secondary" onClick={()=>setDetail(row)}>详情</button></td></tr>)}</tbody></table></div>}
  {requests.hasNextPage?<button className="secondary" disabled={requests.isFetchingNextPage||requests.isError} onClick={()=>void requests.fetchNextPage()}>加载更多</button>:null}
  {detail?<RequestDetail row={detail} onClose={()=>setDetail(undefined)}/>:null}
  </section>;
}
