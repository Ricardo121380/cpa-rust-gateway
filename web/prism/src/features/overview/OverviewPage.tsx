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
import { StatusBadge } from "../../components/StatusBadge";
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
            不可用 —— 该端点需要 P12 之后的网关构建。
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
      <h3 data-gap="top">
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
        来自 <span className="mono">listOperationalBilling</span> 自带的汇总,
        <strong>覆盖整个账本窗口</strong>而不是某一页 —— 所以只取 1 行也是准确的。
        本卡不受顶栏所选配置版本影响。
      </p>
      {summary === undefined ? (
        <p className="stat-sub">读取中…</p>
      ) : summary.records === 0 ? (
        <p className="muted">账本还没有记录 —— 网关尚未处理过可计费的请求。</p>
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
      <h3>带时间维度的分析</h3>
      <p className="stat-sub">
        契约<strong>没有服务端时间桶</strong>,也没有延迟与请求成败清单,所以这里既没有趋势线,
        也没有今日 KPI 与延迟分位 —— 上面的计数器是<strong>累计值</strong>。
        <br />
        按 Provider / 模型 / Client Key 的用量需要跟着游标读到底才准,
        用量分析页会那样做并在提前停止时说明;这里放一个近似值只会和它打架。
      </p>
      <Link to="/usage">前往用量分析 →</Link>
      <br />
      {/* The "recent failures" card that used to carry this link was part of
          the proposed analytics shape. The pointer survives it: failure
          attribution is where that question is actually answerable. */}
      <Link to="/monitoring?tab=failures">在失败归因中查看 →</Link>
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
    <section>
      <h2>{t.nav.overview}</h2>

      <div className="overview-grid">
        <div className="card">
          <h3>活动版本</h3>
          {active !== undefined ? (
            <p>
              <span className="mono">{active.id}</span> <StatusBadge status="active" />
              <br />
              <span className="idchip mono">{active.revision}</span>{" "}
              <span className="muted small">{active.description}</span>
            </p>
          ) : (
            <p className="muted">尚无活动版本 —— 发布一个草稿后出现。</p>
          )}
          <Link to="/versions">前往配置版本 →</Link>
        </div>

        <div className="card">
          <h3>布线规模{scope === undefined ? "(未选择版本)" : ""}</h3>
          {scope === undefined ? (
            <p className="muted">在顶栏选择一个配置版本后显示。</p>
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
      </div>

      <LiveCountersSection />

      <div className="overview-grid" data-gap="top">
        <BillingGlance />
        <AnalyticsPointers />
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
