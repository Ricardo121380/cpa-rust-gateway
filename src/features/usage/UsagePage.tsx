// 用量分析 (docs/07 §7.2). Six tabs over ONE composite analytics query: the
// visible tab decides `include`, so a tab that shows no heatmap never asks the
// backend to compute one.
//
// Chart discipline (docs/07 §7.2, dataviz):
//  - one measure, ONE y axis. Comparing requests against tokens is done with
//    small multiples on 总览, never with a second y scale. The entity-comparison
//    chart on the rank tabs also uses one axis — its series are the SAME measure
//    for different entities, which is exactly what makes them comparable;
//  - the heatmap is a single-hue lightness ramp; the status pool stays reserved
//    for state and never becomes "series 5";
//  - every chart carries a hover layer, an accessible name and a table view.
//
// Value-free: only closed enums and identifiers the backend already returned
// ever reach the query or the URL.
import { useQueries, useQuery } from "@tanstack/react-query";
import { type ReactNode } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { analyticsAvailable, fetchProposedAnalytics } from "../../api/proposed";
import type { AnalyticsFilters, AnalyticsResponse } from "../../api/proposed-types";
import { Heatmap, HeatLegend } from "../../components/data/Heatmap";
import { LineChart } from "../../components/data/LineChart";
import { MultiLineChart, type Series } from "../../components/data/MultiLineChart";
import { RankTable } from "../../components/data/RankTable";
import { SeriesLegend } from "../../components/data/SeriesLegend";
import { formatCount, formatLatency, StatTile } from "../../components/data/StatTile";
import { ZoomBrush } from "../../components/data/ZoomBrush";
import { useMessages } from "../../i18n/messages";
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
  COMPARE_LIMIT,
  compareFilters,
  compareKeys,
  applyZoom,
  findBucketIndex,
  parseSelectedBucket,
  parseZoom,
  zoomAvailable,
  zoomParam,
  type ZoomWindow,
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
  type CompareKind,
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
  const t = useMessages();
  const [params, setParams] = useSearchParams();
  const nowMs = useNowTick();
  const range = paramsToRange(params, nowMs);
  const tab = parseTab(params.get("tab"));
  const metric = parseMetric(params.get("metric"));
  const status = parseStatus(params.get("status"));
  const model = params.get("model");
  const selectedCell = parseCell(params.get("cell"));
  const filters = buildFilters(status, model);
  // Compare panel state rides in the URL like every other filter, so a shared
  // link reproduces exactly what the sender was looking at.
  const compareOpen = params.get("compare") === "1";
  // Zoom is clamped against the actual bucket count, which is only known after
  // the response arrives; parsed below once `data` exists.
  // `at`, not `bucket`: ?bucket= is already the time-granularity parameter in
  // the shared range contract (utils/timerange), and reusing it would make a
  // selected bucket silently change the axis resolution.
  const selectedMs = parseSelectedBucket(params.get("at"));

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
        <h2>{t.nav.usage}</h2>
        <div className="card empty-state" data-kind="unwired">
          <p>
            {t.state.unwired}
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
        <h2>{t.nav.usage}</h2>
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
            <p>{t.state.unwired}</p>
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
            zoom={parseZoom(params.get("z"), (data.timeline ?? []).length)}
            selectedMs={selectedMs}
            onMetric={(next) => updateParams({ metric: next === "requests" ? null : next })}
            onZoom={(next) => updateParams({ z: zoomParam(next) })}
            onSelect={(startMs) => updateParams({ at: startMs === null ? null : String(startMs) })}
          />
        ) : tab === "models" || tab === "clientKeys" || tab === "credentials" ? (
          <RankTab
            data={data}
            kind={tab}
            filtered={filtered}
            hrefFor={(key) =>
              monitoringHref({ from_ms: range.from_ms, to_ms: range.to_ms }, status, key)
            }
            compare={
              <CompareCard
                kind={tab}
                keys={compareKeys(data.ranks)}
                baseFilters={filters}
                range={range}
                metric={metric}
                open={compareOpen}
                onToggle={() => updateParams({ compare: compareOpen ? null : "1" })}
                onMetric={(next) => updateParams({ metric: next === "requests" ? null : next })}
              />
            }
            detail={(row) => (
              <RankDetail
                kind={tab}
                entityKey={row.key}
                baseFilters={filters}
                range={range}
                metric={metric}
              />
            )}
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
          {/* Inside the card, not after the grid: out there its only backdrop was
              the ambient gradient, which measured 2.2:1 — content never sits on
              ambient (DESIGN.md §9 rule 3). */}
          <p className="card-note">
            小倍数:两图各自独立纵轴,量纲不同的指标不叠加到双 Y 轴。
          </p>
        </div>
      </div>

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
  zoom,
  selectedMs,
  onMetric,
  onZoom,
  onSelect,
}: Readonly<{
  data: AnalyticsResponse;
  bucket: "hour" | "day";
  metric: UsageMetric;
  filtered: boolean;
  zoom: ZoomWindow | null;
  selectedMs: number | null;
  onMetric: (metric: UsageMetric) => void;
  onZoom: (next: ZoomWindow | null) => void;
  onSelect: (startMs: number | null) => void;
}>) {
  const timeline = data.timeline ?? [];
  if (timeline.length === 0) {
    return <EmptyPanel filtered={filtered} />;
  }
  // The zoom re-derives the visible window from the buckets already fetched; it
  // never refetches, so the axis is a subset of the same data rather than a
  // different resolution of it.
  const visible = applyZoom(timeline, zoom);
  const points = visible.map((point) => ({
    t: point.bucket_start_ms,
    v: metricValue(point, metric),
  }));
  const selectedIndex = findBucketIndex(visible, selectedMs);
  const selectedBucket = selectedIndex === null ? undefined : visible[selectedIndex];
  const showZoom = zoomAvailable(timeline.length);

  return (
    <div className="card" data-gap="top">
      <div className="card-head">
        <h3>
          {METRIC_LABELS[metric]}(按{bucket === "hour" ? "小时" : "天"},
          {zoom === null
            ? `${timeline.length} 个桶`
            : `${visible.length} / ${timeline.length} 个桶`}
          )
        </h3>
        <MetricSwitch metric={metric} onMetric={onMetric} />
      </div>
      <LineChart
        points={points}
        valueLabel={METRIC_LABELS[metric]}
        formatValue={(value) => formatMetric(value, metric)}
        formatTime={(ms) => axisTime(ms, bucket)}
        ariaLabel={`${METRIC_LABELS[metric]}趋势,按${bucket === "hour" ? "小时" : "天"}`}
        selected={selectedIndex}
        onSelect={(index) => onSelect(visible[index]?.bucket_start_ms ?? null)}
      />
      <p className="card-note">
        单指标单轴;切换指标即切换整张图的纵轴,不做双轴叠加。
        {showZoom ? " 桶数超过 12,可用下方范围条收窄(只重取已加载的窗口,不重新请求)。" : ""}
      </p>

      {showZoom ? (
        <ZoomBrush
          values={timeline.map((point) => metricValue(point, metric))}
          start={zoom?.start ?? 0}
          end={zoom?.end ?? timeline.length - 1}
          formatIndex={(index) =>
            fullTime(timeline[index]?.bucket_start_ms ?? 0, bucket)
          }
          onChange={(next) => onZoom(next)}
          onReset={() => onZoom(null)}
        />
      ) : null}

      {selectedBucket === undefined ? (
        <p className="usage-hint">点击图上任意一点(或键盘方向键 + Enter)标记一个桶。</p>
      ) : (
        <div className="bucket-detail">
          <div className="bucket-detail-head">
            <strong className="mono">{fullTime(selectedBucket.bucket_start_ms, bucket)}</strong>
            <button type="button" className="chip-off" onClick={() => onSelect(null)}>
              取消选中
            </button>
          </div>
          <dl className="bucket-facts">
            <dt>{METRIC_LABELS[metric]}</dt>
            <dd className="mono">{formatMetric(metricValue(selectedBucket, metric), metric)}</dd>
            <dt>请求 / 失败</dt>
            <dd className="mono">
              {formatCount(selectedBucket.requests)} / {formatCount(selectedBucket.failures)}
            </dd>
            <dt>Token</dt>
            <dd className="mono">{formatCount(selectedBucket.tokens_total)}</dd>
            <dt>P95</dt>
            <dd className="mono">{formatLatency(selectedBucket.latency_p95_ms ?? null)}</dd>
          </dl>
        </div>
      )}

      <details className="chart-table">
        <summary>
          数据表({visible.length} 行{zoom === null ? "" : ",已按范围条收窄"})
        </summary>
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
              {visible.map((point) => (
                <tr
                  key={point.bucket_start_ms}
                  data-selected={
                    point.bucket_start_ms === selectedMs ? "true" : undefined
                  }
                >
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

// ---------- 模型 / Client Key / 凭据 ----------

const RANK_COPY: Readonly<Record<CompareKind, Readonly<{ title: string; keyLabel: string }>>> = {
  models: { title: "模型排行(按请求数)", keyLabel: "公开模型" },
  clientKeys: { title: "Client Key 排行(按请求数)", keyLabel: "Client Key" },
  credentials: { title: "凭据排行(按请求数)", keyLabel: "凭据" },
};

function RankTab({
  data,
  kind,
  filtered,
  hrefFor,
  compare,
  detail,
}: Readonly<{
  data: AnalyticsResponse;
  kind: CompareKind;
  filtered: boolean;
  hrefFor: (key: string) => string;
  compare: ReactNode;
  detail: (row: Readonly<{ key: string }>) => ReactNode;
}>) {
  const rows = data.ranks ?? [];
  if (rows.length === 0) {
    return <EmptyPanel filtered={filtered} />;
  }
  const copy = RANK_COPY[kind];

  return (
    <>
      <div className="card tablewrap" data-gap="top">
        <h3>{copy.title}</h3>
        <RankTable
          rows={rows}
          keyLabel={copy.keyLabel}
          detail={detail}
          action={
            kind === "models"
              ? (row) => (
                  <Link className="usage-link" to={hrefFor(row.key)}>
                    查看请求 →
                  </Link>
                )
              : undefined
          }
        />
        {kind === "models" ? null : (
          <p className="usage-hint">
            请求监控当前只接受时间窗与模型两个过滤维度,
            {kind === "clientKeys" ? "Client Key" : "凭据"}维度的下钻在 G3 补齐对应过滤后开放。
          </p>
        )}
      </div>
      {compare}
    </>
  );
}

// ---------- 展开对比面板(单实体明细) ----------

/** The row's own summary + shape, from the same endpoint with this one entity
 *  pinned. Mounted only when the row is open, so a closed table issues nothing. */
function RankDetail({
  kind,
  entityKey,
  baseFilters,
  range,
  metric,
}: Readonly<{
  kind: CompareKind;
  entityKey: string;
  baseFilters: AnalyticsFilters;
  range: Readonly<{ from_ms: number; to_ms: number; bucket: Bucket }>;
  metric: UsageMetric;
}>) {
  const detail = useQuery({
    queryKey: [
      "usage-rank-detail",
      kind,
      entityKey,
      range.from_ms,
      range.to_ms,
      range.bucket,
      JSON.stringify(baseFilters),
    ],
    queryFn: () =>
      fetchProposedAnalytics({
        from_ms: range.from_ms,
        to_ms: range.to_ms,
        timezone: TIMEZONE,
        bucket: range.bucket,
        filters: compareFilters(baseFilters, kind, entityKey),
        include: { summary: true, timeline: true },
      }),
    staleTime: 30_000,
  });

  if (detail.isPending) {
    return <p className="card-note">加载 {entityKey} 的明细…</p>;
  }
  if (detail.isError || detail.data === undefined) {
    return <p className="card-note">该实体的明细查询失败 —— 上方排行数据仍然有效。</p>;
  }

  const summary = detail.data.summary;
  const timeline = detail.data.timeline ?? [];
  if (summary === undefined || summary.requests === 0) {
    return <p className="card-note">此窗口内该实体没有可展开的明细。</p>;
  }

  return (
    <div className="rank-detail">
      <dl className="rank-detail-facts">
        <dt>请求 / 尝试</dt>
        <dd className="mono">
          {formatCount(summary.requests)} / {formatCount(summary.attempts)}
        </dd>
        <dt>成功率</dt>
        <dd className="mono">
          {(((summary.requests - summary.failures) / summary.requests) * 100).toFixed(2)}%
        </dd>
        <dt>Token</dt>
        <dd className="mono">{formatCount(tokensTotal(summary.tokens))}</dd>
        <dt>缓存读</dt>
        <dd className="mono">{formatCount(summary.tokens.cache_read ?? 0)}</dd>
        <dt>P95 / P99</dt>
        <dd className="mono">
          {formatLatency(summary.latency_ms.p95)} / {formatLatency(summary.latency_ms.p99)}
        </dd>
      </dl>
      {timeline.length === 0 ? null : (
        <div className="rank-detail-chart">
          <LineChart
            points={timeline.map((point) => ({
              t: point.bucket_start_ms,
              v: metricValue(point, metric),
            }))}
            valueLabel={METRIC_LABELS[metric]}
            formatValue={(value) => formatMetric(value, metric)}
            formatTime={(ms) => axisTime(ms, detail.data.range.bucket)}
            ariaLabel={`${entityKey} 的${METRIC_LABELS[metric]}趋势`}
            compact
          />
        </div>
      )}
    </div>
  );
}

// ---------- 实体对比(top-N 多线,固定色序) ----------

/** One query per entity: the contract's timeline is unsegmented, so N series
 *  means N filtered windows. Off by default — it is N extra round trips, and the
 *  rank table above already answers "who is biggest". Opening it is the user
 *  saying they want the shape over time. */
function CompareCard({
  kind,
  keys,
  baseFilters,
  range,
  metric,
  open,
  onToggle,
  onMetric,
}: Readonly<{
  kind: CompareKind;
  keys: readonly string[];
  baseFilters: AnalyticsFilters;
  range: Readonly<{ from_ms: number; to_ms: number; bucket: Bucket }>;
  metric: UsageMetric;
  open: boolean;
  onToggle: () => void;
  onMetric: (next: UsageMetric) => void;
}>) {
  const results = useQueries({
    queries: keys.map((key) => ({
      queryKey: [
        "usage-compare",
        kind,
        key,
        range.from_ms,
        range.to_ms,
        range.bucket,
        JSON.stringify(baseFilters),
      ],
      queryFn: () =>
        fetchProposedAnalytics({
          from_ms: range.from_ms,
          to_ms: range.to_ms,
          timezone: TIMEZONE,
          bucket: range.bucket,
          filters: compareFilters(baseFilters, kind, key),
          include: { timeline: true },
        }),
      enabled: open && analyticsAvailable(),
      staleTime: 30_000,
    })),
  });

  const loading = open && results.some((result) => result.isPending);
  const failed = open && results.some((result) => result.isError);
  const series: readonly Series[] = keys
    .map((key, index) => {
      const timeline = results[index]?.data?.timeline ?? [];
      return {
        key,
        label: key,
        points: timeline.map((point) => ({
          t: point.bucket_start_ms,
          v: metricValue(point, metric),
        })),
      };
    })
    .filter((one) => one.points.length > 0);

  return (
    <div className="card" data-gap="top">
      <div className="card-head">
        <h3>实体对比(前 {COMPARE_LIMIT})</h3>
        <button type="button" className={open ? "chip-on" : "chip-off"} onClick={onToggle}>
          {open ? "收起对比" : "展开对比"}
        </button>
      </div>
      {!open ? (
        <p className="card-note">
          按排名取前 {COMPARE_LIMIT} 项,各自单独查询一次(契约的 timeline 不分段),
          共用一根纵轴直接比较。因为要多发 {COMPARE_LIMIT} 次请求,默认收起。
        </p>
      ) : failed ? (
        <p className="card-note">对比查询失败 —— 排行表仍然可用。</p>
      ) : loading ? (
        <p className="card-note">加载 {keys.length} 条序列…</p>
      ) : series.length === 0 ? (
        <p className="card-note">前 {COMPARE_LIMIT} 项在此窗口内没有可对比的时间序列。</p>
      ) : (
        <>
          <div className="usage-chips" role="group" aria-label="对比指标">
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
          <SeriesLegend items={series} />
          <MultiLineChart
            series={series}
            valueLabel={METRIC_LABELS[metric]}
            formatValue={(value) => formatMetric(value, metric)}
            formatTime={(ms) => axisTime(ms, range.bucket === "day" ? "day" : "hour")}
            ariaLabel={`${RANK_COPY[kind].keyLabel}对比,${METRIC_LABELS[metric]}`}
          />
        </>
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
  const t = useMessages();
  return (
    <div className="card empty-state" data-kind="empty" data-gap="top">
      <p>{filtered ? t.state.filteredEmpty : t.state.empty}</p>
    </div>
  );
}
