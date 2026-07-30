// 用量分析 (docs/07 §7.2). Five tabs over ONE composite analytics query: the
// visible tab decides `include`, so a tab that shows no heatmap never asks the
// backend to compute one.
//
// Chart discipline (docs/07 §7.2, dataviz):
//  - one measure, ONE y axis. Comparing requests against tokens is done with
//    small multiples on 总览, never with a second y scale;
//  - the heatmap is a single-hue lightness ramp; the status pool stays reserved
//    for state and never becomes "series 5";
//  - every chart carries a hover layer, an accessible name and a table view.
//
// Value-free: only closed enums and identifiers the backend already returned
// ever reach the query or the URL.
import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";
import { analyticsAvailable, fetchProposedAnalytics } from "../../api/proposed";
import type { AnalyticsResponse } from "../../api/proposed-types";
import { Heatmap, HeatLegend } from "../../components/data/Heatmap";
import { LineChart } from "../../components/data/LineChart";
import { RankTable } from "../../components/data/RankTable";
import { formatCount, formatLatency, StatTile } from "../../components/data/StatTile";
import { messages } from "../../i18n/messages";
import {
  paramsToRange,
  rangeToParams,
  resolvePreset,
  type Bucket,
  type RangePreset,
} from "../../utils/timerange";
import { useNowTick } from "../../utils/useNowTick";
import {
  buildFilters,
  cellParam,
  cellWindow,
  formatAxisNumber,
  formatMetric,
  hasActiveFilter,
  heatBins,
  heatStep,
  includeForTab,
  METRIC_LABELS,
  metricValue,
  monitoringHref,
  parseCell,
  parseMetric,
  parseStatus,
  parseTab,
  TAB_LABELS,
  USAGE_METRICS,
  USAGE_TABS,
  WEEKDAY_LABELS,
  type UsageMetric,
} from "./model";
import "./usage.css";

const PRESETS: ReadonlyArray<{ key: Exclude<RangePreset, "custom">; label: string }> = [
  { key: "today", label: "今天" },
  { key: "24h", label: "24 小时" },
  { key: "7d", label: "7 天" },
  { key: "30d", label: "30 天" },
];

const TIMEZONE = Intl.DateTimeFormat().resolvedOptions().timeZone;

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

function axisTime(ms: number, bucket: "hour" | "day"): string {
  const date = new Date(ms);
  return bucket === "hour" ? `${pad(date.getHours())}:00` : `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function fullTime(ms: number, bucket: "hour" | "day"): string {
  const date = new Date(ms);
  const day = `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  return bucket === "hour" ? `${day} ${pad(date.getHours())}:00` : day;
}

function tokensTotal(tokens: Readonly<Record<string, number | undefined>>): number {
  return Object.values(tokens).reduce<number>((sum, value) => sum + (value ?? 0), 0);
}

export function UsagePage() {
  const [params, setParams] = useSearchParams();
  const nowMs = useNowTick();
  const range = paramsToRange(params, nowMs);
  const tab = parseTab(params.get("tab"));
  const metric = parseMetric(params.get("metric"));
  const status = parseStatus(params.get("status"));
  const model = params.get("model");
  const selectedCell = parseCell(params.get("cell"));
  const filters = buildFilters(status, model);

  const detailWindow =
    selectedCell === null
      ? null
      : cellWindow(selectedCell.weekday, selectedCell.hour, range.from_ms, range.to_ms);

  // One composite query per tab. `useNowTick` quantises "now" so the key is
  // stable between ticks — a raw Date.now() here would rebuild the key on every
  // render and the page would refetch forever.
  const analytics = useQuery({
    queryKey: [
      "usage",
      tab,
      metric,
      range.from_ms,
      range.to_ms,
      range.bucket,
      status,
      model,
    ],
    queryFn: () =>
      fetchProposedAnalytics({
        from_ms: range.from_ms,
        to_ms: range.to_ms,
        timezone: TIMEZONE,
        bucket: range.bucket,
        filters,
        include: includeForTab(tab, metric),
      }),
    enabled: analyticsAvailable(),
    placeholderData: (previous) => previous,
    staleTime: 30_000,
  });

  // The revealed cell asks the SAME endpoint for its own hour — the window the
  // panel describes is exactly the window the deep link carries.
  const cellDetail = useQuery({
    queryKey: ["usage-cell", detailWindow?.from_ms, detailWindow?.to_ms, status, model],
    queryFn: () =>
      fetchProposedAnalytics({
        from_ms: detailWindow!.from_ms,
        to_ms: detailWindow!.to_ms,
        timezone: TIMEZONE,
        bucket: "hour",
        filters,
        include: { summary: true, ranks: { by: "public_model", limit: 5 } },
      }),
    enabled: analyticsAvailable() && tab === "heatmap" && detailWindow !== null,
    staleTime: 30_000,
  });

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

  if (!analyticsAvailable()) {
    return (
      <section>
        <h2>{messages.nav.usage}</h2>
        <div className="card empty-state" data-kind="unwired">
          <p>
            {messages.state.unwired}
            <br />
            <small className="muted-3">
              趋势、排行与热力图全部由单个 POST /admin/analytics(G3)驱动 —— 端点交付后本页整体点亮。
            </small>
          </p>
        </div>
      </section>
    );
  }

  const data = analytics.data;
  const modelOptions = data?.options?.["public_model"] ?? [];
  const filtered = hasActiveFilter(status, model);

  return (
    <section>
      <header className="page-head">
        <h2>{messages.nav.usage}</h2>
      </header>

      <div className="usage-tabbar">
        <div className="usage-tabs" role="tablist" aria-label="用量分析子页">
          {USAGE_TABS.map((key) => (
            <button
              key={key}
              type="button"
              role="tab"
              id={`usage-tab-${key}`}
              aria-selected={tab === key}
              aria-controls="usage-panel"
              onClick={() => updateParams({ tab: key === "overview" ? null : key })}
            >
              {TAB_LABELS[key]}
            </button>
          ))}
        </div>
      </div>

      <div className="filter-bar card">
        <div className="preset-chips" role="group" aria-label="时间范围">
          {PRESETS.map((preset) => (
            <button
              key={preset.key}
              type="button"
              className={range.preset === preset.key ? "chip-on" : "chip-off"}
              onClick={() =>
                updateParams({ ...rangeToParams(resolvePreset(preset.key, Date.now())), from: null, to: null })
              }
            >
              {preset.label}
            </button>
          ))}
        </div>
        <label>
          粒度
          <select
            value={range.bucket}
            onChange={(event) =>
              updateParams({ bucket: event.target.value === "auto" ? null : (event.target.value as Bucket) })
            }
          >
            <option value="auto">自动</option>
            <option value="hour">小时</option>
            <option value="day">天</option>
          </select>
        </label>
        <label>
          状态
          <select
            value={status}
            onChange={(event) =>
              updateParams({ status: event.target.value === "all" ? null : event.target.value })
            }
          >
            <option value="all">全部</option>
            <option value="success">仅成功</option>
            <option value="failed">仅失败</option>
          </select>
        </label>
        <label>
          模型
          <select
            value={model ?? ""}
            onChange={(event) => updateParams({ model: event.target.value === "" ? null : event.target.value })}
          >
            <option value="">全部模型</option>
            {modelOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div role="tabpanel" id="usage-panel" aria-labelledby={`usage-tab-${tab}`}>
        {analytics.isError ? (
          <div className="card empty-state" data-kind="unwired" data-gap="top">
            <p>{messages.state.unwired}</p>
          </div>
        ) : data === undefined ? (
          <div className="card empty-state" data-kind="empty" data-gap="top">
            <p>加载分析投影…</p>
          </div>
        ) : tab === "overview" ? (
          <OverviewTab data={data} bucket={data.range.bucket} filtered={filtered} />
        ) : tab === "trend" ? (
          <TrendTab
            data={data}
            bucket={data.range.bucket}
            metric={metric}
            filtered={filtered}
            onMetric={(next) => updateParams({ metric: next === "requests" ? null : next })}
          />
        ) : tab === "models" || tab === "credentials" ? (
          <RankTab
            data={data}
            kind={tab}
            filtered={filtered}
            hrefFor={(key) =>
              monitoringHref({ from_ms: range.from_ms, to_ms: range.to_ms }, status, key)
            }
          />
        ) : (
          <HeatmapTab
            data={data}
            metric={metric}
            filtered={filtered}
            selected={selectedCell}
            detail={cellDetail.data}
            detailWindow={detailWindow}
            monitoringTo={
              detailWindow === null ? null : monitoringHref(detailWindow, status, model)
            }
            onMetric={(next) => updateParams({ metric: next === "requests" ? null : next })}
            onSelect={(cell) => updateParams({ cell: cellParam(cell) })}
          />
        )}
      </div>
    </section>
  );
}

// ---------- 总览 ----------

function OverviewTab({
  data,
  bucket,
  filtered,
}: Readonly<{ data: AnalyticsResponse; bucket: "hour" | "day"; filtered: boolean }>) {
  const kpi = data.summary;
  const timeline = data.timeline ?? [];
  const ranks = data.ranks ?? [];

  if (kpi === undefined || kpi.requests === 0) {
    return <EmptyPanel filtered={filtered} />;
  }

  return (
    <>
      <div className="stat-row">
        <StatTile
          label="请求"
          value={formatCount(kpi.requests)}
          sub={`尝试 ${formatCount(kpi.attempts)}`}
          spark={timeline.map((point) => point.requests)}
        />
        <StatTile
          label="成功率"
          value={`${(((kpi.requests - kpi.failures) / kpi.requests) * 100).toFixed(2)}%`}
          sub={`失败 ${formatCount(kpi.failures)}`}
        />
        <StatTile
          label="Token"
          value={formatCount(tokensTotal(kpi.tokens))}
          sub={`缓存读 ${formatCount(kpi.tokens.cache_read ?? 0)}`}
        />
        <StatTile
          label="P95 / P99"
          value={formatLatency(kpi.latency_ms.p95)}
          sub={`P99 ${formatLatency(kpi.latency_ms.p99)}`}
        />
      </div>

      <div className="overview-grid" data-gap="top">
        <div className="card">
          <div className="card-head">
            <h3>请求数</h3>
          </div>
          <LineChart
            points={timeline.map((point) => ({ t: point.bucket_start_ms, v: point.requests }))}
            valueLabel="请求"
            formatValue={formatAxisNumber}
            formatTime={(ms) => axisTime(ms, bucket)}
            ariaLabel={`请求数趋势,按${bucket === "hour" ? "小时" : "天"}`}
            compact
          />
        </div>
        <div className="card">
          <div className="card-head">
            <h3>Token</h3>
          </div>
          <LineChart
            points={timeline.map((point) => ({ t: point.bucket_start_ms, v: point.tokens_total }))}
            valueLabel="Token"
            formatValue={formatAxisNumber}
            formatTime={(ms) => axisTime(ms, bucket)}
            ariaLabel={`Token 趋势,按${bucket === "hour" ? "小时" : "天"}`}
            compact
          />
        </div>
      </div>
      <p className="card-note">
        小倍数:两图各自独立纵轴,量纲不同的指标不叠加到双 Y 轴。
      </p>

      <div className="card tablewrap" data-gap="top">
        <h3>模型汇总(窗口内)</h3>
        <RankTable rows={ranks} keyLabel="公开模型" />
      </div>
    </>
  );
}

// ---------- 趋势 ----------

function TrendTab({
  data,
  bucket,
  metric,
  filtered,
  onMetric,
}: Readonly<{
  data: AnalyticsResponse;
  bucket: "hour" | "day";
  metric: UsageMetric;
  filtered: boolean;
  onMetric: (metric: UsageMetric) => void;
}>) {
  const timeline = data.timeline ?? [];
  if (timeline.length === 0) {
    return <EmptyPanel filtered={filtered} />;
  }
  const points = timeline.map((point) => ({
    t: point.bucket_start_ms,
    v: metricValue(point, metric),
  }));

  return (
    <div className="card" data-gap="top">
      <div className="card-head">
        <h3>
          {METRIC_LABELS[metric]}(按{bucket === "hour" ? "小时" : "天"},{timeline.length} 个桶)
        </h3>
        <MetricSwitch metric={metric} onMetric={onMetric} />
      </div>
      <LineChart
        points={points}
        valueLabel={METRIC_LABELS[metric]}
        formatValue={(value) => formatMetric(value, metric)}
        formatTime={(ms) => axisTime(ms, bucket)}
        ariaLabel={`${METRIC_LABELS[metric]}趋势,按${bucket === "hour" ? "小时" : "天"}`}
      />
      <p className="card-note">单指标单轴;切换指标即切换整张图的纵轴,不做双轴叠加。</p>

      <details className="chart-table">
        <summary>数据表({timeline.length} 行)</summary>
        <div className="chart-table-scroll">
          <table>
            <thead>
              <tr>
                <th>时间</th>
                <th>{METRIC_LABELS[metric]}</th>
                <th>请求</th>
                <th>失败</th>
                <th>Token</th>
              </tr>
            </thead>
            <tbody>
              {timeline.map((point) => (
                <tr key={point.bucket_start_ms}>
                  <td className="mono">{fullTime(point.bucket_start_ms, bucket)}</td>
                  <td className="mono">{formatMetric(metricValue(point, metric), metric)}</td>
                  <td className="mono">{formatCount(point.requests)}</td>
                  <td className="mono">{formatCount(point.failures)}</td>
                  <td className="mono">{formatCount(point.tokens_total)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </div>
  );
}

function MetricSwitch({
  metric,
  onMetric,
}: Readonly<{ metric: UsageMetric; onMetric: (metric: UsageMetric) => void }>) {
  return (
    <div className="usage-chips" role="group" aria-label="指标">
      {USAGE_METRICS.map((key) => (
        <button
          key={key}
          type="button"
          className={metric === key ? "chip-on" : "chip-off"}
          aria-pressed={metric === key}
          onClick={() => onMetric(key)}
        >
          {METRIC_LABELS[key]}
        </button>
      ))}
    </div>
  );
}

// ---------- 模型 / 凭据 ----------

function RankTab({
  data,
  kind,
  filtered,
  hrefFor,
}: Readonly<{
  data: AnalyticsResponse;
  kind: "models" | "credentials";
  filtered: boolean;
  hrefFor: (key: string) => string;
}>) {
  const rows = data.ranks ?? [];
  if (rows.length === 0) {
    return <EmptyPanel filtered={filtered} />;
  }
  const isModels = kind === "models";

  return (
    <div className="card tablewrap" data-gap="top">
      <h3>{isModels ? "模型排行(按请求数)" : "凭据排行(按请求数)"}</h3>
      <RankTable
        rows={rows}
        keyLabel={isModels ? "公开模型" : "凭据"}
        action={
          isModels
            ? (row) => (
                <Link className="usage-link" to={hrefFor(row.key)}>
                  查看请求 →
                </Link>
              )
            : undefined
        }
      />
      {isModels ? null : (
        <p className="usage-hint">
          请求监控当前只接受时间窗与模型两个过滤维度,凭据维度的下钻在 G3 补齐凭据过滤后开放。
        </p>
      )}
    </div>
  );
}

// ---------- 热力图 ----------

function HeatmapTab({
  data,
  metric,
  filtered,
  selected,
  detail,
  detailWindow,
  monitoringTo,
  onMetric,
  onSelect,
}: Readonly<{
  data: AnalyticsResponse;
  metric: UsageMetric;
  filtered: boolean;
  selected: { weekday: number; hour: number } | null;
  detail: AnalyticsResponse | undefined;
  detailWindow: { from_ms: number; to_ms: number } | null;
  monitoringTo: string | null;
  onMetric: (metric: UsageMetric) => void;
  onSelect: (cell: { weekday: number; hour: number }) => void;
}>) {
  const cells = data.heatmap ?? [];
  if (cells.length === 0) {
    return <EmptyPanel filtered={filtered} />;
  }
  const peak = cells.reduce((best, cell) => (cell.value > best ? cell.value : best), 0);
  const format = (value: number) => formatMetric(value, metric);

  return (
    <>
      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>星期 × 小时 · {METRIC_LABELS[metric]}</h3>
          <MetricSwitch metric={metric} onMetric={onMetric} />
        </div>
        <Heatmap
          cells={cells}
          weekdayLabels={WEEKDAY_LABELS}
          stepOf={(value) => heatStep(value, peak)}
          formatValue={format}
          metricLabel={METRIC_LABELS[metric]}
          selected={selected}
          onSelect={onSelect}
        />
        <HeatLegend bins={heatBins(peak)} formatValue={format} />
        <p className="card-note">单色相顺序色带;点击格子展开该小时的明细与下钻入口。</p>
      </div>

      {selected === null ? (
        <p className="usage-hint">尚未选择格子。</p>
      ) : (
        <div className="card" data-gap="top">
          <div className="heat-detail-head">
            <h3>
              {WEEKDAY_LABELS[selected.weekday]} {pad(selected.hour)}:00
            </h3>
            <span className="heat-detail-window">
              {detailWindow === null
                ? "当前时间范围未覆盖该格子"
                : `${fullTime(detailWindow.from_ms, "hour")} → ${fullTime(detailWindow.to_ms, "hour")}(本范围内最近一次)`}
            </span>
          </div>

          {detailWindow === null ? (
            <p className="usage-hint">换用更长的时间范围后,该格子才有可下钻的具体窗口。</p>
          ) : detail?.summary === undefined ? (
            <p className="stat-sub">加载该小时明细…</p>
          ) : (
            <>
              <div className="heat-kpis">
                <span className="heat-kpi">
                  <span className="stat-label">请求</span>
                  <span className="stat-value mono">{formatCount(detail.summary.requests)}</span>
                </span>
                <span className="heat-kpi">
                  <span className="stat-label">失败</span>
                  <span className="stat-value mono">{formatCount(detail.summary.failures)}</span>
                </span>
                <span className="heat-kpi">
                  <span className="stat-label">Token</span>
                  <span className="stat-value mono">{formatCount(tokensTotal(detail.summary.tokens))}</span>
                </span>
                <span className="heat-kpi">
                  <span className="stat-label">P95</span>
                  <span className="stat-value mono">{formatLatency(detail.summary.latency_ms.p95)}</span>
                </span>
              </div>
              {(detail.ranks ?? []).length > 0 ? (
                <RankTable rows={detail.ranks ?? []} keyLabel="贡献模型" />
              ) : null}
            </>
          )}

          {monitoringTo !== null ? (
            <Link className="usage-drill" to={monitoringTo}>
              在请求监控中查看该小时 →
            </Link>
          ) : null}
        </div>
      )}
    </>
  );
}

// ---------- empty ----------

function EmptyPanel({ filtered }: Readonly<{ filtered: boolean }>) {
  return (
    <div className="card empty-state" data-kind="empty" data-gap="top">
      <p>{filtered ? messages.state.filteredEmpty : messages.state.empty}</p>
    </div>
  );
}
