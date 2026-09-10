import { ProcessingStatus } from "../billing/ProcessingStatus";
// Overview. Three truth layers, honestly separated (docs/07 §7.1):
//  - wiring scale: real counts from the existing list contracts, per version;
//  - live counters: the REAL bounded Prometheus exposition (G2 partial) —
//    cumulative since gateway start, no time window, no per-entity split;
//  - time-dimension analytics: still the PROPOSED G3 shapes (fixtures only),
//    otherwise the dedicated "pipeline unwired" state.
import { useQuery } from "@tanstack/react-query";
import { useRef } from "react";
import { Link } from "react-router-dom";
import { call, callText } from "../../api/client";
import { asAppError } from "../../api/errors";
import {
  exactShare,
  formatPercent,
  type BillingResponse,
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
          <span className="badge badge-good">必需事件无丢失</span>
        ) : (
          <span className="badge badge-critical">
            {formatCount(counters.requiredLoss)} 条必需事件丢失
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
        <p className="stat-sub">队列未拒绝必需事件,写入器未隔离也未写失败 —— 事件日志完整。</p>
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

  if (metrics.isError) {
    return (
      <div className="card empty-state" data-kind="unwired" data-gap="top">
        <p>
          网关未提供观测指标
          <br />
          <small className="muted">
            <span className="mono">GET /admin/observability/metrics</span>{" "}
            不可用，请检查网关是否提供观测接口。
          </small>
        </p>
      </div>
    );
  }
  if (metrics.data === undefined) {
    return (
      <div className="card empty-state" data-kind="empty" data-gap="top">
        <p>读取网关计数器…</p>
      </div>
    );
  }

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
 * There is still no latency and no success rate anywhere in the contract.
 */
function BillingGlance() {
  const billing = useQuery({
    // Not version-scoped, like the monitoring ledger it summarises.
    queryKey: ["overview-billing"],
    queryFn: () => call<BillingResponse>("listOperationalBilling", { query: { limit: 1 } }),
    retry: false,
    refetchInterval: 60_000,
  });

  if (billing.isError) {
    return (
      <div className="card empty-state" data-kind="error" data-gap="top">
        <p>{asAppError(billing.error).message}</p>
      </div>
    );
  }

  const summary = billing.data?.summary;

  return (
    <div className="card" data-gap="top">
      <h3>计价可信度</h3>
      <p className="stat-sub">
        覆盖整个账本窗口 · 跨配置版本。金额与计价置信度以已处理的账本为准。
      </p>
      {summary === undefined ? (
        <p className="stat-sub">读取中…</p>
      ) : summary.records === 0 ? (
        <p className="muted">账本暂无记录。可能尚未处理或没有可计价事件，不能据此判断没有消费。</p>
      ) : (
        <div className="count-row">
          <span className="count-tile">
            <span className="count-value mono">{formatCount(summary.records)}</span>
            <span className="count-label">账本记录</span>
          </span>
          <span className="count-tile">
            <span className="count-value mono">{formatPercent(exactShare(summary))}</span>
            <span className="count-label">成本精确</span>
          </span>
          <span className="count-tile">
            <span className="count-value mono">{formatCount(summary.unpriced_records)}</span>
            <span className="count-label">无价格</span>
          </span>
        </div>
      )}
      <Link to="/monitoring">前往请求监控 →</Link>
    </div>
  );
}

function AnalyticsPointers() {
  return (
    <div className="card" data-gap="top">
      <h3>继续查看</h3>
      <div className="overview-shortcuts"><Link to="/accounts">账号池 →</Link><Link to="/catalog">模型目录 →</Link><Link to="/usage">前往用量分析 →</Link><Link to="/monitoring?tab=failures">在失败归因中查看 →</Link></div>
      <details className="reading-notes"><summary>分析范围</summary><p>计数器为累计值；当前没有服务端时间桶，也没有请求延迟分布。用量页提供所选时间窗的聚合，并说明观测范围是否完整。</p></details>
    </div>
  );
}

export function OverviewPage() {
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
  const egress = useCount("egress-count", "listEgressPolicies", scope);
  const models = useCount("models-count", "listPublicModels", scope);
  const groups = useCount("groups-count", "listAccessGroups", scope);
  const keys = useCount("keys-count", "listClientKeys", scope);

  const counts: ReadonlyArray<{ label: string; to: string; value: number | undefined }> = [
    { label: "上游", to: "/upstreams", value: upstreams.data },
    { label: "出口策略", to: "/egress", value: egress.data },
    { label: "公开模型", to: "/models", value: models.data },
    { label: "访问组", to: "/access", value: groups.data },
    { label: "Client Key", to: "/access", value: keys.data },
  ];

  return (
    <section className="overview-page">
      <header className="page-head"><h2>{t.nav.overview}</h2><Link to="/monitoring">查看请求 →</Link></header>
      <LiveCountersSection />
      <div className="overview-workspace">
        <div className="overview-primary">
          <ProcessingStatus compact />
          <BillingGlance />
          <AnalyticsPointers />
        </div>
        <aside className="overview-aside">
          <ReadStatus pending={versions.isPending} error={versions.error} hasData={versions.data !== undefined} retry={() => void versions.refetch()} />
          <div className="card overview-resources">
          <div className="overview-resource-head"><h3>资源概览</h3><Link to="/versions">{context?.status === "draft" ? "当前草稿" : context?.status === "archived" ? "历史配置" : active === undefined ? "配置待初始化" : "已发布配置"} →</Link></div>
          {scope === undefined ? (
            <p className="muted">请到“配置版本”发布或选择一份配置。</p>
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
        </aside>
      </div>
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
