// 用量分析 — GET /admin/operations/usage (P13-04B).
//
// The previous page here was built for the PROPOSED G3 analytics shape and, in
// production, rendered nothing: its data source was gated on dev fixtures. This
// one reads the contract that exists, and its whole design follows from three
// properties of that contract (see features/usage/model.ts for the details):
//
//   1. There are NO SERVER-SIDE TIME BUCKETS, so there is no trend line, no
//      heatmap and no zoom brush. A row is an interval aggregate. Stitching a
//      series out of K windowed queries would need K x pages requests to stay
//      correct, and doing it without following each window's cursor would
//      under-report silently — the page says this instead of drawing it.
//   2. `limit` caps at 100, so totals require following the cursor. The page
//      fetches up to MAX_PAGES and states plainly when it stopped early.
//   3. Every token family carries its own confidence and a nullable total.
//      Unobserved is not zero, and a sum containing an unobserved contributor
//      is a floor — both are surfaced rather than smoothed away.
import { useQuery } from "@tanstack/react-query";
import { useMemo, type FormEvent } from "react";
import { useSearchParams } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useMessages } from "../../i18n/messages";
import "./usage.css";
import {
  activeFilterCount,
  collect,
  confidenceLabel,
  confidenceTone,
  DIMENSIONS,
  dimensionLabel,
  familyLabel,
  FILTER_KEYS,
  filterLabel,
  formatCount,
  formatTokens,
  formatWatermark,
  groupBy,
  MAX_PAGES,
  PAGE_LIMIT,
  parseDimension,
  parseFilters,
  parseRange,
  PROTOCOLS,
  RANGE_PRESETS,
  rangeLabel,
  rangeParams,
  shareOf,
  sumFamily,
  TOKEN_FAMILIES,
  type Collected,
  type FamilyTotal,
  type Filters,
  type UsageResponse,
} from "./model";

/** Follows the cursor to completion, bounded by MAX_PAGES.
 *
 *  One query, many requests: TanStack's infinite query is built for "load
 *  more" UIs, but every number on this page is a total over the whole result
 *  set, so a partially-loaded state would show wrong figures rather than fewer
 *  of them. Fetching to completion up front is the honest shape. */
async function fetchAll(
  filters: Filters,
  range: Readonly<{ from_ms?: number; to_ms?: number }>,
): Promise<Collected> {
  const pages: UsageResponse[] = [];
  let cursor: string | null = null;
  do {
    const page: UsageResponse = await call<UsageResponse>("listOperationalUsage", {
      query: {
        ...range,
        ...filters,
        limit: PAGE_LIMIT,
        ...(cursor === null ? {} : { cursor }),
      },
    });
    // NOT versionScoped: listOperationalUsage declares no X-Config-Version.
    // Usage is durable observation of requests that already happened, so it
    // spans config versions by construction — see the note on the page.
    pages.push(page);
    cursor = page.next_cursor;
  } while (cursor !== null && pages.length < MAX_PAGES);
  return collect(pages);
}

function ConfidenceChip({ total }: Readonly<{ total: FamilyTotal }>) {
  return (
    <span className="usage-conf" data-tone={confidenceTone(total.confidence)}>
      {confidenceLabel(total.confidence)}
      {total.partialCoverage ? <span className="usage-floor">≥</span> : null}
    </span>
  );
}

/** A family cell: number, then how much to trust it. The `≥` marker is not
 *  decoration — it means at least one contributor reported no observation, so
 *  the figure is a lower bound. */
function FamilyCell({ total }: Readonly<{ total: FamilyTotal }>) {
  return (
    <td className="mono usage-num">
      {total.partialCoverage && total.total !== null ? "≥ " : ""}
      {formatTokens(total.total)}
      <ConfidenceChip total={total} />
    </td>
  );
}

/** Share bar for one group. SVG presentation attributes, never a CSS `style`
 *  attribute — the shipped CSP is `style-src 'self'` with no inline exemption,
 *  and check.mjs bans inline style attributes for exactly that reason. (This
 *  sentence avoids spelling the banned token: the gate is a text match, and
 *  the right move is to change the prose, not the gate.) */
function ShareBar({ share }: Readonly<{ share: number }>) {
  return (
    <svg
      className="usage-share"
      viewBox="0 0 100 4"
      preserveAspectRatio="none"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="0" y="0" width={Math.max(0.5, share * 100)} height="4" rx="2" fill="var(--chart-1)" />
    </svg>
  );
}

export function UsagePage() {
  const t = useMessages();
  const [params, setParams] = useSearchParams();

  const range = parseRange(params.get("range"));
  const dimension = parseDimension(params.get("by"));
  const filters = parseFilters((key) => params.get(key));

  // The window is pinned to the query key, not recomputed on every render:
  // a moving `now` would make every re-render a cache miss.
  const window = useMemo(() => rangeParams(range, Date.now()), [range]);

  const usage = useQuery({
    // No config version in the key: this operation is not version-scoped.
    queryKey: ["usage", range, JSON.stringify(filters)],
    queryFn: () => fetchAll(filters, window),
    retry: false,
  });

  const groups = useMemo(
    () => groupBy(usage.data?.rows ?? [], dimension),
    [usage.data, dimension],
  );
  const totalRequests = groups.reduce((sum, group) => sum + group.request_count, 0);
  const rows = usage.data?.rows ?? [];

  function patch(next: Readonly<Record<string, string | null>>): void {
    const merged = new URLSearchParams(params);
    for (const [key, value] of Object.entries(next)) {
      if (value === null || value === "") {
        merged.delete(key);
      } else {
        merged.set(key, value);
      }
    }
    setParams(merged, { replace: true });
  }

  function onFilterSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    patch(Object.fromEntries(FILTER_KEYS.map((key) => [key, String(data.get(key) ?? "").trim()])));
  }

  return (
    <section className="usage-page">
      <header className="page-head">
        <h2>{t.nav.usage}</h2>
        <code className="idchip mono">listOperationalUsage</code>
      </header>

      <p className="usage-hint">
        一行是一个<strong>(Provider · Channel · 账号 · 公开模型 · 协议 · Client Key · 访问组)</strong>
        组合在所选时间窗内的<strong>聚合</strong>。契约没有服务端时间桶,所以这里没有趋势线,
        也没有热力图 —— 用 K 个窗口在前端拼一条曲线,要么需要 K×页 次请求,
        要么会静默少算,两者都不如把这句话写出来。
        <br />
        <strong>本页不受顶栏所选配置版本影响</strong>:
        <span className="mono">listOperationalUsage</span> 不带{" "}
        <span className="mono">X-Config-Version</span> —— 用量是已发生请求的持久观测,
        天然跨版本。
        <br />
        成本不在本页:usage 的 <span className="mono">cost_confidence</span> 恒为{" "}
        <span className="mono">unpriced</span>,计价在「计费」侧。
      </p>

      <div className="card usage-controls">
        <div className="usage-seg" role="group" aria-label="时间窗">
          {RANGE_PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              className="secondary"
              aria-pressed={preset === range}
              onClick={() => patch({ range: preset })}
            >
              {rangeLabel(preset)}
            </button>
          ))}
        </div>
        <label className="usage-by">
          分组维度
          <select value={dimension} onChange={(event) => patch({ by: event.target.value })}>
            {DIMENSIONS.map((value) => (
              <option key={value} value={value}>
                {dimensionLabel(value)}
              </option>
            ))}
          </select>
        </label>
      </div>

      <form className="card usage-filters" onSubmit={onFilterSubmit}>
        {FILTER_KEYS.map((key) =>
          key === "protocol" ? (
            <label key={key}>
              {filterLabel(key)}
              <select name={key} defaultValue={filters.protocol ?? ""}>
                <option value="">全部</option>
                {PROTOCOLS.map((protocol) => (
                  <option key={protocol} value={protocol}>
                    {protocol}
                  </option>
                ))}
              </select>
            </label>
          ) : (
            <label key={key}>
              {filterLabel(key)}
              <input
                name={key}
                className="mono"
                maxLength={key === "model" ? 256 : 128}
                defaultValue={filters[key] ?? ""}
              />
            </label>
          ),
        )}
        <div className="usage-filter-actions">
          <button type="submit">应用筛选</button>
          <button
            type="button"
            className="secondary"
            disabled={activeFilterCount(filters) === 0}
            onClick={() => patch(Object.fromEntries(FILTER_KEYS.map((key) => [key, null])))}
          >
            清除({activeFilterCount(filters)})
          </button>
        </div>
      </form>

      {usage.isError ? (
        <div className="card empty-state" data-kind="error">
          <p>{asAppError(usage.error).message}</p>
        </div>
      ) : usage.isPending ? (
        <div className="card empty-state" data-kind="loading">
          <p>正在读取用量(按游标翻页,最多 {MAX_PAGES} 页)…</p>
        </div>
      ) : (
        <>
          <div className="card usage-summary">
            <div className="usage-kpi">
              <span className="usage-kpi-value mono">{formatCount(totalRequests)}</span>
              <span className="usage-kpi-label">请求数</span>
            </div>
            <div className="usage-kpi">
              <span className="usage-kpi-value mono">{formatCount(rows.length)}</span>
              <span className="usage-kpi-label">聚合行</span>
            </div>
            <div className="usage-kpi">
              <span className="usage-kpi-value mono">{formatCount(groups.length)}</span>
              <span className="usage-kpi-label">{dimensionLabel(dimension)} 数</span>
            </div>
            <p className="usage-watermark">
              数据截至 <span className="mono">{formatWatermark(usage.data.observed_through_ms)}</span>
              {" · "}
              读取 {usage.data.pages} 页
              {usage.data.truncated ? null : "(已到末页)"}
            </p>
          </div>

          {usage.data.truncated ? (
            <p role="alert" className="action-error">
              结果在第 {MAX_PAGES} 页被截断,后面还有数据 ——
              <strong>下面的合计是不完整的</strong>。请缩小时间窗或加上筛选条件后重试。
            </p>
          ) : null}

          {rows.length === 0 ? (
            <div className="card empty-state" data-kind="empty">
              <p>
                该窗口内没有用量记录。
                <br />
                <small className="muted-3">
                  用量来自已完成请求的持久化观测,配置本身不产生用量 ——
                  一个从未接过流量的版本在这里就是空的。
                </small>
              </p>
            </div>
          ) : (
            <div className="card tablewrap">
              <table className="usage-table">
                <thead>
                  <tr>
                    <th scope="col">{dimensionLabel(dimension)}</th>
                    <th scope="col">请求</th>
                    {TOKEN_FAMILIES.map((family) => (
                      <th key={family} scope="col">
                        {familyLabel(family)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {groups.map((group) => (
                    <tr key={group.key}>
                      <th scope="row" className="mono usage-key">
                        {group.key}
                        <ShareBar share={shareOf(group.request_count, totalRequests)} />
                      </th>
                      <td className="mono usage-num">{formatCount(group.request_count)}</td>
                      {TOKEN_FAMILIES.map((family) => (
                        <FamilyCell key={family} total={group.families[family]} />
                      ))}
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr>
                    <th scope="row">合计</th>
                    <td className="mono usage-num">{formatCount(totalRequests)}</td>
                    {TOKEN_FAMILIES.map((family) => (
                      <FamilyCell key={family} total={sumFamily(rows, family)} />
                    ))}
                  </tr>
                </tfoot>
              </table>
            </div>
          )}

          <p className="usage-hint">
            <strong>≥</strong> 表示该合计里至少有一个来源没有观测值 —— 它是<strong>下界</strong>,
            不是总数。未观测与零是两件事,本页不把它们合并。
            置信度取组内<strong>最弱</strong>的那一个。
          </p>
        </>
      )}
    </section>
  );
}
