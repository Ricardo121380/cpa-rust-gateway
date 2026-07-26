// Overview. Two truth layers, honestly separated (docs/07 §7.1):
//  - wiring scale: real counts from the existing list contracts, per version;
//  - observability: lit from the PROPOSED G3 dashboard/analytics (fixtures)
//    until G2/G3 land — otherwise the dedicated "pipeline unwired" state.
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
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
import { messages } from "../../i18n/messages";
import { resolvePreset } from "../../utils/timerange";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../config-versions/versionStore";

function useCount(queryKey: string, operation: Parameters<typeof call>[0], scope: string | undefined) {
  return useQuery({
    queryKey: [queryKey, scope],
    queryFn: async () => ((await call<unknown[]>(operation, {}, { versionScoped: true })) ?? []).length,
    enabled: scope !== undefined,
    staleTime: 30_000,
  });
}

function ObservabilitySection() {
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
          {messages.state.unwired}
          <br />
          <small className="muted-3">
            今日 KPI、流量趋势、健康条带与 Token 构成将在 G2(事件管道接线)与
            G3(分析端点)交付后点亮 —— 归后端会话。
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
      <h2>{messages.nav.overview}</h2>

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

      <ObservabilitySection />
    </section>
  );
}

export function PlaceholderPage({ title }: Readonly<{ title: string }>) {
  return (
    <section>
      <h2>{title}</h2>
      <div className="card empty-state" data-kind="empty">
        <p>{messages.state.empty}</p>
      </div>
    </section>
  );
}
