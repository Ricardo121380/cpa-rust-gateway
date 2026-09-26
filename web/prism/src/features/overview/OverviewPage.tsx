import { PagedReadStatus } from "../../components/PagedReadStatus";
import { useAccountDirectory } from "../accounts/useAccountDirectory";
import { accountGroups } from "../accounts/presentation";
import type { PoolSnapshot } from "../runtime/model";
import { useOverviewBilling } from "./useOverviewBilling";
import { ProcessingStatus } from "../billing/ProcessingStatus";
import { RequestOverview } from "../monitoring/RequestHistory";
import { useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { Link } from "react-router-dom";
import { call, callText } from "../../api/client";
import { asAppError } from "../../api/errors";
import {
  exactShare,
  summaryIsPartitioned,
} from "../monitoring/model";
import { formatCount, StatTile } from "../../components/data/StatTile";
import { TokenMixBar } from "../../components/data/TokenMixBar";
import { ReadStatus } from "../../components/ReadStatus";
import { useMessages } from "../../i18n/messages";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../config-versions/versionStore";
import { growthSince, readCounters, successRate, type GatewayCounters } from "./metrics";
import { StorageCapacity } from "./StorageCapacity";

const METRICS_POLL_MS = 15_000;

function useCount(queryKey: string, operation: Parameters<typeof call>[0], scope: string | undefined) {
  return useQuery({
    queryKey: [queryKey, scope],
    queryFn: async () => ((await call<unknown[]>(operation, {}, { versionScoped: true })) ?? []).length,
    enabled: scope !== undefined,
    staleTime: 30_000,
  });
}

function EventMix({ counters }: Readonly<{ counters: GatewayCounters }>) {
  const ROWS = [
    { kind: "request", label: "请求" },
    { kind: "attempt", label: "上游尝试" },
    { kind: "usage", label: "用量" },
    { kind: "health", label: "健康" },
    { kind: "diagnostic", label: "诊断" },
  ] as const;

  return (
    <div className="card">
      <h3>事件构成</h3>
      <table>
        <tbody>
          {ROWS.map((row) => (
            <tr key={row.kind}>
              <td>{row.label}</td>
              <td className="mono">{formatCount(counters.events[row.kind])}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="stat-sub">消费者按类别处理的事件数,不含请求内容。</p>
    </div>
  );
}

function PipelineHealth({ counters }: Readonly<{ counters: GatewayCounters }>) {
  const required = counters.loss.filter((signal) => signal.severity === "required");

  return (
    <div className="card">
      <h3>
        观测管道健康{" "}
        {counters.requiredLoss === 0 ? (
          <span className="badge badge-good">本进程未见记录告警</span>
        ) : (
          <span className="badge badge-critical">
            {formatCount(counters.requiredLoss)} 次记录告警
          </span>
        )}
      </h3>
      {required.length > 0 ? (
        <table>
          <tbody>
            {required.map((signal) => (
              <tr key={signal.key}>
                <td>{signal.label}</td>
                <td className="mono">{formatCount(signal.value)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="stat-sub">本进程计数中未见队列拒绝、隔离或写入失败；这不证明全部历史记录完整。</p>
      )}
      <p className="stat-sub">
        待写入 <span className="mono">{counters.pendingRequired}</span> 条
        {counters.diagnosticLoss > 0
          ? ` · 诊断事件已丢弃 ${formatCount(counters.diagnosticLoss)} 条(背压设计,非故障)`
          : ""}
      </p>
    </div>
  );
}

function LiveCountersSection() {
  const metrics = useQuery({
    queryKey: ["observability-metrics"],
    queryFn: () => callText("getObservabilityMetrics"),
    refetchInterval: METRICS_POLL_MS,
    retry: false,
  });

  // Baseline for this visit. The counters are cumulative over the gateway
  // process, so the lifetime number says little about now — the delta is what
  // is actually being watched. The scrape time is stored with it: before a
  // second scrape there is no observation window at all, and "+0" then is a
  // claim about a window that never happened. After one, "+0" is real news.
  const baseline = useRef<{ counters: GatewayCounters; at: number } | undefined>(undefined);

  if (metrics.data === undefined) return <PagedReadStatus query={metrics}/>;

  const counters = readCounters(metrics.data);
  baseline.current ??= { counters, at: metrics.dataUpdatedAt };
  const growth =
    metrics.dataUpdatedAt > baseline.current.at
      ? growthSince(baseline.current.counters, counters)
      : undefined;
  const rate = successRate(counters);
  const visit = (value: number): string =>
    growth === undefined ? "" : ` · 本页 +${formatCount(value)}`;

  return (
    <>
      <PagedReadStatus query={metrics}/>
      <section className="card" aria-label="请求记录状态" role={counters.recording.accepting===false?"alert":undefined}>
        <h3>请求记录 · {counters.recording.accepting===null?"状态未观测":counters.recording.accepting?"可接收新请求":"暂停接收新请求"}</h3>
        <p>{({0:"正在启动恢复",1:"持久化可用",2:"存储暂不可用",3:"记录系统已关闭"} as Record<number,string>)[counters.recording.state??-1]??"记录系统状态未知"} · 待确认 {counters.recording.pending??"未观测"}</p>
        <p className="stat-sub">最后成功写入：{counters.recording.lastCommit?new Date(counters.recording.lastCommit).toLocaleString():"未观测"} · 确认失败 {counters.recording.confirmationFailures??"未观测"} 次 · 启动恢复为终态未知 {counters.recording.recoveredUnknown??"未观测"} 条</p>
        {counters.recording.accepting===false?<p>记录系统恢复后才会接受新请求。请检查运行日志与存储状态；普通服务健康不代表记录可用。</p>:null}
      </section>
      <StorageCapacity capacity={counters.capacity}/>
      <details className="overview-telemetry"><summary>进程计数与运行事件</summary>
      <h3 className="overview-metrics-title" data-gap="top">
        网关实时计数 <span className="badge badge-muted">自进程启动累计</span>
      </h3>

      <div className="stat-row">
        <StatTile
          label="上游尝试"
          value={formatCount(counters.attempts.total)}
          sub={`失败 ${formatCount(counters.attempts.failed)}${visit(growth?.attempts ?? 0)}`}
        />
        <StatTile
          label="尝试成功率"
          value={rate === undefined ? "—" : `${(rate * 100).toFixed(2)}%`}
          sub={
            rate === undefined
              ? "尚未观测到任何尝试"
              : `${formatCount(counters.attempts.succeeded)}/${formatCount(counters.attempts.total)}`
          }
        />
        <StatTile
          label="已处理事件"
          value={formatCount(counters.eventsTotal)}
          sub={`用量 ${formatCount(counters.events.usage)}${visit(growth?.events ?? 0)}`}
        />
      </div>

      <details className="overview-telemetry"><summary>事件、Token 与观测管道</summary>
      <div className="overview-grid" data-gap="top">
        <EventMix counters={counters} />
        {/* Cumulative, and now unconditional: the "today" bar it used to step
            aside for was part of the proposed analytics shape and never
            existed. */}
        <div className="card">
          <h3>Token 构成(累计)</h3>
          <TokenMixBar tokens={counters.tokens} />
        </div>
        <PipelineHealth counters={counters} />
      </div>
      </details>
      </details>
    </>
  );
}

/**
 * The observability half of this page used to be a "today" dashboard over the
 * PROPOSED G3 analytics shape: today's KPIs, an hourly trend, a health strip, a
 * model ranking, latency percentiles. None of it existed outside dev fixtures,
 * so in production this whole area rendered a single "not wired yet" card.
 *
 * What replaced it is deliberately small, because only two things can be shown
 * here honestly and cheaply:
 *
 *   - The BILLING SUMMARY is one request and its figures cover the whole
 *     window, not the page (the backend accumulates before the cursor applies).
 *     That makes it the one real KPI this page can carry.
 *   - Everything else worth showing needs the cursor followed to the end.
 *     用量分析 does exactly that and says when it had to stop; repeating a
 *     one-page approximation here would contradict it. So this links there
 *     instead of showing a partial sum that looks authoritative.
 *
 * Persisted request terminals now provide a separate, snapshot-consistent
 * request view with timing and percentiles. The overview links there instead
 * of mixing those observations with process-lifetime counters.
 */
function BillingGlance({range}:Readonly<{range:{from_ms:number;to_ms:number}}>) {
  const billing = useOverviewBilling(range);

  if (billing.isError) {
    return (
      <div className="card empty-state" data-kind="error" data-gap="top">
        <p>{asAppError(billing.error).kind === "unavailable" ? "费用概览暂时无法读取。" : asAppError(billing.error).message}</p>
        <button type="button" className="secondary" onClick={() => void billing.refetch()}>重新读取</button>
      </div>
    );
  }

  const summary = billing.data?.summary;

  return (
    <div className="card overview-billing">
      <h3>费用完整性</h3>
      <p className="stat-sub">
        与请求概览使用同一时间范围；账本可能稍后完成处理。
      </p>
      {summary === undefined ? (
        <p className="stat-sub">读取中…</p>
      ) : summary.records === 0 ? (
        <p className="muted">账本暂无记录。可能尚未处理或没有可计价事件，不能据此判断没有消费。</p>
      ) : (
        <>
          <div className="billing-completeness-number">{((exactShare(summary)??0)*100).toFixed(1)}<small>%</small></div>
          <p className="stat-sub">账本记录的精确计价占比</p>
          {summaryIsPartitioned(summary)?<svg className="billing-completeness-track" viewBox="0 0 100 6" preserveAspectRatio="none" role="img" aria-label="计价记录构成；各类别数量见下方">{([
            ["exact",summary.exact_records], ["partial",summary.partial_records], ["unknown",summary.unknown_records], ["unpriced",summary.unpriced_records],
          ] as const).map(([kind,count],index,parts)=><rect key={kind} className={`billing-part-${kind}`} x={parts.slice(0,index).reduce((sum,part)=>sum+part[1],0)/summary.records*100} y="0" width={count/summary.records*100} height="6"/>)}</svg>:<p className="stat-sub">计价分类数量暂不一致，请核对账本。</p>}
          <dl className="billing-completeness-facts">{([
            ["exact","精确计价",summary.exact_records], ["partial","部分已知",summary.partial_records], ["unknown","用量未知",summary.unknown_records], ["unpriced","尚未计价",summary.unpriced_records],
          ] as const).map(([kind,label,count])=><div key={kind}><dt><span className={`billing-dot billing-part-${kind}`} aria-hidden="true"/>{label}</dt><dd>{formatCount(count)} 条</dd></div>)}</dl>
          <p className="stat-sub">账本共 {formatCount(summary.records)} 条，可能晚于请求完成。未计价不是零费用。</p>
        </>
      )}
      <Link to={`/monitoring?tab=ledger&from_ms=${range.from_ms}&to_ms=${range.to_ms}`}>查看费用与计价详情 →</Link>
    </div>
  );
}

function ProviderAccountsGlance() {
  const inventory=useAccountDirectory({q:"",category:"",status:"",sort:"name",upstream_id:""});
  const context=useVersionStore(state=>state.context);
  const summary=inventory.data?.pages[0];
  const runtime=useQuery({queryKey:["overview-account-runtime"],queryFn:()=>call<PoolSnapshot>("listProviderAccountPools",{query:{limit:100}}),retry:false,refetchInterval:30_000});
  return <section className="card overview-providers">
    <header className="overview-resource-head"><h3>提供商与账号</h3><Link to="/accounts">管理账号 →</Link></header>
    <p className="stat-sub">{context?.status==="draft"?"待应用配置":context?.status==="archived"?"历史配置":"当前配置"}凭据与独立渠道账号；以下为授权数量，非运行可用数。</p>
    <ReadStatus pending={runtime.isPending} error={runtime.error} hasData={runtime.data!==undefined} retry={()=>void runtime.refetch()}/>
    {runtime.data?<p className="stat-sub">运行绑定{runtime.data.next_cursor?"（仅已载入部分）":""}：{runtime.data.items.length} 个 · 认证可用 {runtime.data.items.filter(item=>item.auth_status==="active").length} · 调度可用 {runtime.data.items.filter(item=>item.runtime_status==="available").length} · 冷却 {runtime.data.items.filter(item=>item.runtime_status==="cooling").length}。同一账号的多个接口分别计数。<Link to="/accounts?view=runtime">查看运行状态 →</Link></p>:null}
    {context===undefined||context===null?<p className="muted">尚无配置上下文，请先接入提供商。</p>:<ReadStatus pending={inventory.isPending} error={inventory.error} hasData={summary!==undefined} retry={()=>void inventory.refetch()}/>}
    {summary?summary.total===0?<p className="muted">尚未接入账号。</p>:<div className="overview-provider-rows">{accountGroups.filter(group=>(summary.category_totals[group.id]??0)>0).map(group=><Link key={group.id} to={`/accounts?category=${encodeURIComponent(group.id)}`}><span className="account-avatar" aria-hidden="true">{group.name.slice(0,2)}</span><span><strong>{group.name}</strong><small>{group.description}</small></span><strong>{summary.category_totals[group.id]} <small>份授权</small></strong><span aria-hidden="true">→</span></Link>)}</div>:null}
  </section>;
}

function AnalyticsPointers() {
  return (
    <div className="card" data-gap="top">
      <h3>继续查看</h3>
      <div className="overview-shortcuts"><Link to="/accounts">账号池 →</Link><Link to="/catalog">模型目录 →</Link><Link to="/usage">前往用量分析 →</Link><Link to="/monitoring?tab=failures">在失败归因中查看 →</Link></div>
      <details className="reading-notes"><summary>分析范围</summary><p>计数器为进程累计值；请求日志提供独立的时间桶、终态和延迟观测。用量页提供所选时间窗的聚合，并说明观测范围是否完整。</p></details>
    </div>
  );
}

export function OverviewPage() {
  const [requestRange,setRequestRange]=useState(()=>({from_ms:Date.now()-86_400_000,to_ms:Date.now()}));
  const t = useMessages();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;

  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 30_000,
  });
  const active = versions.data?.find((row) => row.status === "active");

  const upstreams = useCount("upstreams-count", "listUpstreams", scope);
  const models = useCount("models-count", "listPublicModels", scope);
  const keys = useCount("keys-count", "listClientKeys", scope);

  const counts: ReadonlyArray<{ label: string; to: string; value: number | undefined }> = [
    { label: "提供商", to: "/upstreams", value: upstreams.data },
    { label: "公开模型", to: "/models", value: models.data },
    { label: "客户端密钥", to: "/access", value: keys.data },
  ];

  return (
    <section className="overview-page">
      {!versions.isPending&&(!active||upstreams.data===0||models.data===0||keys.data===0)?<div className="card setup-guide"><h3>开始使用</h3><ol className="setup-steps"><li><Link to="/upstreams?add=provider"><strong>1. 接入提供商</strong><span>设置接口地址并添加账号</span></Link></li><li><Link to="/models?add=model"><strong>2. 开放模型</strong><span>选择模型与接口连接</span></Link></li><li><Link to="/access"><strong>3. 创建客户端密钥</strong><span>选择允许使用的模型</span></Link></li></ol></div>:null}
      <RequestOverview title={t.nav.overview} onRangeChange={setRequestRange}/>
      <div className="overview-workspace">
        <div className="overview-primary">
          <ProviderAccountsGlance />

        </div>
        <aside className="overview-aside">
          <BillingGlance range={requestRange} />
        </aside>
      </div>
      <details className="overview-telemetry overview-maintenance"><summary>资源与处理状态</summary><ProcessingStatus compact /><AnalyticsPointers />
          <ReadStatus pending={versions.isPending} error={versions.error} hasData={versions.data !== undefined} retry={() => void versions.refetch()} />
          <div className="card overview-resources">
          <div className="overview-resource-head"><h3>资源概览</h3><span className="entity-meta">{context?.status === "draft" ? "待应用" : context?.status === "archived" ? "历史配置" : active === undefined ? "等待接入" : "当前配置"}</span></div>
          {scope === undefined ? null : <ReadStatus
            pending={upstreams.isPending || models.isPending || keys.isPending}
            error={upstreams.error ?? models.error ?? keys.error}
            hasData={counts.every(item => item.value !== undefined)}
            retry={() => { void Promise.all([upstreams.refetch(), models.refetch(), keys.refetch()]); }}
          />}
          {scope === undefined ? (
            <Link to="/upstreams?add=provider">接入第一个提供商 →</Link>
          ) : (
            <div className="count-row">
              {counts.map((item) => (
                <Link key={item.label} to={item.to} className="count-tile">
                  <span className="count-value mono">{item.value ?? "…"}</span>
                  <span className="count-label">{item.label}</span>
                </Link>
              ))}
            </div>
          )}
          </div>
      </details>
      <LiveCountersSection/>
    </section>
  );
}

export function PlaceholderPage({ title }: Readonly<{ title: string }>) {
  const t = useMessages();
  return (
    <section>
      <h2>{title}</h2>
      <div className="card empty-state" data-kind="empty">
        <p>{t.state.empty}</p>
      </div>
    </section>
  );
}
