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
import {
  analyticsAvailable,
  fetchProposedAnalytics,
  fetchProposedDashboard,
} from "../../api/proposed";
import { HealthStrip } from "../../components/data/HealthStrip";
import { MiniTimeline } from "../../components/data/MiniTimeline";
import { formatCount, formatLatency, StatTile } from "../../components/data/StatTile";
import { TokenMixBar } from "../../components/data/TokenMixBar";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { resolvePreset } from "../../utils/timerange";
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
        {/* The cumulative mix is the fallback view. Once G3 lands, its
            same-shaped "today" bar is strictly more useful, and two token
            cards on one page is just noise — so this one steps aside. */}
        {analyticsAvailable() ? null : (
          <div className="card">
            <h3>Token 构成(累计)</h3>
            <TokenMixBar tokens={counters.tokens} />
          </div>
        )}
        <PipelineHealth counters={counters} />
      </div>
    </>
  );
}

function ObservabilitySection() {
  const t = useMessages();
  const today = resolvePreset("today", Date.now());

  const dashboard = useQuery({
    queryKey: ["dashboard-summary"],
    queryFn: () => fetchProposedDashboard(today.from_ms, today.to_ms),
    enabled: analyticsAvailable(),
    refetchInterval: 60_000,
  });

  const trend = useQuery({
    queryKey: ["overview-trend", today.from_ms],
    queryFn: () =>
      fetchProposedAnalytics({
        from_ms: today.from_ms,
        to_ms: today.to_ms,
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        bucket: "hour",
        include: { timeline: true },
      }),
    enabled: analyticsAvailable(),
    refetchInterval: 60_000,
  });

  if (!analyticsAvailable()) {
    return (
      <div className="card empty-state" data-kind="unwired" data-gap="top">
        <p>
          {t.state.unwired}
          <br />
          <small className="muted">
            上面的计数器是累计值。带时间维度的部分 —— 今日 KPI、流量趋势、健康条带、
            模型排行与延迟分位 —— 需要 G3 分析端点(时间桶 + 按实体切分),
            指标曝露刻意不带这些标签,无法由它推导。归后端会话。
          </small>
        </p>
      </div>
    );
  }
  const summary = dashboard.data;
  if (summary === undefined) {
    return (
      <div className="card empty-state" data-kind="empty" data-gap="top">
        <p>加载今日观测…</p>
      </div>
    );
  }

  return (
    <>
      {/* This plane must announce itself: its 成功率 is today's, the counters
          plane above shows the process lifetime's, and two unlabelled stat
          rows with different numbers for the same word read as a bug. */}
      <h3 data-gap="top">
        今日分析 <span className="badge badge-muted">按时间窗</span>
      </h3>

      <div className="stat-row">
        <StatTile
          label="今日请求"
          value={formatCount(summary.kpi.requests)}
          sub={`失败 ${summary.kpi.failures}`}
          spark={trend.data?.timeline?.map((bucket) => bucket.requests)}
        />
        <StatTile
          label="成功率"
          value={`${(summary.kpi.success_rate * 100).toFixed(2)}%`}
          sub={`${summary.kpi.requests - summary.kpi.failures}/${summary.kpi.requests}`}
        />
        <StatTile label="Token" value={formatCount(summary.kpi.tokens_total)} sub="全部类别合计" />
        <StatTile label="P95 延迟" value={formatLatency(summary.kpi.latency_p95_ms)} sub="来源:尝试时间戳" />
      </div>

      <div className="overview-grid" data-gap="top">
        <div className="card">
          <h3>流量趋势(今日,按小时)</h3>
          {trend.data?.timeline !== undefined ? (
            <MiniTimeline buckets={trend.data.timeline} />
          ) : (
            <p className="stat-sub">加载中…</p>
          )}
        </div>
        <div className="card">
          <h3>Token 构成(今日)</h3>
          <TokenMixBar tokens={summary.token_mix} />
        </div>
      </div>

      <div className="card" data-gap="top">
        <h3>请求健康条带(10 分钟桶)</h3>
        <HealthStrip buckets={summary.health_strip} />
      </div>

      <div className="overview-grid" data-gap="top">
        <div className="card">
          <h3>模型用量排行(今日)</h3>
          <table>
            <tbody>
              {summary.top_models.map((row) => (
                <tr key={row.public_model}>
                  <td className="mono">{row.public_model}</td>
                  <td className="mono">{formatCount(row.requests)} 请求</td>
                  <td className="mono">{formatCount(row.tokens_total)} tok</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="card">
          <h3>最近失败</h3>
          {summary.recent_failures.length === 0 ? (
            <p className="stat-sub">今日无失败请求</p>
          ) : (
            <table>
              <tbody>
                {summary.recent_failures.map((failure) => (
                  <tr key={failure.request_id}>
                    <td className="mono">{failure.request_id}</td>
                    <td>
                      <StatusBadge status="credential_forbidden">
                        {failure.error_code} · {failure.error_scope}
                      </StatusBadge>
                    </td>
                    <td className="mono">{failure.stage ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <Link to="/monitoring?range=today&status=failed">在请求监控中查看 →</Link>
        </div>
      </div>
    </>
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
      <ObservabilitySection />
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
