// Request monitor (FE-2 MVP): KPI row + filters synced to the URL contract +
// realtime events table with cursor paging. Data comes from the PROPOSED G3
// analytics endpoint (fixtures) until G2/G3 land.
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { analyticsAvailable, fetchProposedAnalytics } from "../../api/proposed";
import type { AnalyticsFilters, RequestEventView } from "../../api/proposed-types";
import { formatCount, formatLatency, StatTile } from "../../components/data/StatTile";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import {
  paramsToRange,
  rangeToParams,
  resolveBucket,
  resolvePreset,
  type RangePreset,
} from "../../utils/timerange";
import { useNowTick } from "../../utils/useNowTick";

const PRESETS: ReadonlyArray<{ key: Exclude<RangePreset, "custom">; label: string }> = [
  { key: "today", label: "今天" },
  { key: "24h", label: "24 小时" },
  { key: "7d", label: "7 天" },
  { key: "30d", label: "30 天" },
];

function outcomeBadge(row: RequestEventView): string {
  return row.outcome === "success" ? "active" : "credential_forbidden";
}

export function MonitoringPage() {
  const t = useMessages();
  const [params, setParams] = useSearchParams();
  const nowMs = useNowTick();
  const range = paramsToRange(params, nowMs);
  const status = (params.get("status") ?? "all") as "all" | "success" | "failed";
  const model = params.get("model");

  const filters: AnalyticsFilters = {
    status,
    ...(model !== null ? { public_model: [model] } : {}),
  };

  function updateParams(next: Record<string, string | null>) {
    const merged = new URLSearchParams(params);
    for (const [key, value] of Object.entries(next)) {
      if (value === null) {
        merged.delete(key);
      } else {
        merged.set(key, value);
      }
    }
    setParams(merged, { replace: true });
  }

  const summary = useQuery({
    queryKey: ["monitoring-summary", range.from_ms, range.to_ms, status, model],
    queryFn: () =>
      fetchProposedAnalytics({
        from_ms: range.from_ms,
        to_ms: range.to_ms,
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        bucket: resolveBucket(range),
        filters,
        include: { summary: true, options: true },
      }),
    enabled: analyticsAvailable(),
    refetchInterval: 10_000,
    placeholderData: (previous) => previous,
  });

  const events = useInfiniteQuery({
    queryKey: ["monitoring-events", range.from_ms, range.to_ms, status, model],
    queryFn: ({ pageParam }) =>
      fetchProposedAnalytics({
        from_ms: range.from_ms,
        to_ms: range.to_ms,
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        bucket: resolveBucket(range),
        filters,
        include: { events: { cursor: pageParam, limit: 25 } },
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.events?.next_cursor ?? null,
    enabled: analyticsAvailable(),
  });

  if (!analyticsAvailable()) {
    return (
      <section>
        <h2>{t.nav.monitoring}</h2>
        <div className="card empty-state" data-kind="unwired">
          <p>{t.state.unwired}</p>
        </div>
      </section>
    );
  }

  const kpi = summary.data?.summary;
  const modelOptions = summary.data?.options?.["public_model"] ?? [];
  const rows = (events.data?.pages ?? []).flatMap((page) => page.events?.items ?? []);

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.monitoring}</h2>
      </header>

      <div className="filter-bar card">
        <div className="preset-chips" role="group" aria-label="时间范围">
          {PRESETS.map((preset) => (
            <button
              key={preset.key}
              type="button"
              className={range.preset === preset.key ? "chip-on" : "chip-off"}
              onClick={() => {
                const next = resolvePreset(preset.key, Date.now());
                updateParams({ ...rangeToParams(next), from: null, to: null });
              }}
            >
              {preset.label}
            </button>
          ))}
        </div>
        <label>
          状态
          <select value={status} onChange={(event) => updateParams({ status: event.target.value === "all" ? null : event.target.value })}>
            <option value="all">全部</option>
            <option value="success">仅成功</option>
            <option value="failed">仅失败</option>
          </select>
        </label>
        <label>
          模型
          <select value={model ?? ""} onChange={(event) => updateParams({ model: event.target.value === "" ? null : event.target.value })}>
            <option value="">全部模型</option>
            {modelOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
      </div>

      {kpi !== undefined ? (
        <div className="stat-row">
          <StatTile label="请求" value={formatCount(kpi.requests)} sub={`尝试 ${formatCount(kpi.attempts)}`} />
          <StatTile
            label="失败"
            value={formatCount(kpi.failures)}
            sub={kpi.requests > 0 ? `${((kpi.failures / kpi.requests) * 100).toFixed(2)}%` : "—"}
          />
          <StatTile
            label="Token"
            value={formatCount(
              Object.values(kpi.tokens).reduce((sum, value) => sum + (value ?? 0), 0),
            )}
            sub={`缓存读 ${formatCount(kpi.tokens.cache_read ?? 0)}`}
          />
          <StatTile label="P95 / P99" value={formatLatency(kpi.latency_ms.p95)} sub={`P99 ${formatLatency(kpi.latency_ms.p99)}`} />
        </div>
      ) : null}

      <div className="card tablewrap">
        <h3>逐请求事件(value-free:标识符与闭集枚举,无请求原文)</h3>
        <table>
          <thead>
            <tr>
              <th>时间</th>
              <th>请求</th>
              <th>模型</th>
              <th>协议</th>
              <th>结果</th>
              <th>阶段</th>
              <th>重试决策</th>
              <th>尝试</th>
              <th>延迟</th>
              <th>Token</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.request_id}>
                <td className="mono">
                  {new Date(row.occurred_at_ms).toISOString().slice(11, 19)}
                </td>
                <td className="mono">{row.request_id}</td>
                <td className="mono">{row.public_model}</td>
                <td className="mono">
                  {row.protocol === "openai_responses" ? "responses" : "messages"}
                  {row.streaming ? " · SSE" : ""}
                </td>
                <td>
                  <StatusBadge status={outcomeBadge(row)}>
                    {row.outcome === "success" ? "成功" : `${row.error_code}`}
                  </StatusBadge>
                </td>
                <td className="mono">{row.stage ?? "—"}</td>
                <td className="mono">{row.retry_decision ?? "—"}</td>
                <td className="mono">{row.attempt_count}</td>
                <td className="mono">{formatLatency(row.latency_ms)}</td>
                <td className="mono">
                  {row.tokens !== null && row.tokens !== undefined
                    ? formatCount(
                        Object.values(row.tokens).reduce((sum, value) => sum + (value ?? 0), 0),
                      )
                    : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && !events.isLoading ? (
          <div className="empty-state" data-kind="empty">
            <p>{status !== "all" || model !== null ? t.state.filteredEmpty : t.state.empty}</p>
          </div>
        ) : null}
        {events.hasNextPage ? (
          <div className="load-more">
            <button
              type="button"
              className="secondary"
              disabled={events.isFetchingNextPage}
              onClick={() => void events.fetchNextPage()}
            >
              加载更多
            </button>
          </div>
        ) : null}
      </div>
    </section>
  );
}
