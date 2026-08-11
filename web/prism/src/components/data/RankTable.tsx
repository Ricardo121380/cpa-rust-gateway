// Rank table with share bars. The bar is MAGNITUDE, so it is one hue for every
// row — colour must never encode rank, otherwise filtering the list repaints
// the survivors and the reader re-learns the chart.
//
// Rows can expand into a detail panel (docs/07 §7.2 "展开对比面板"). Expansion is
// a <tr> spanning every column rather than a nested table, so the browser keeps
// one column grid and the panel stays aligned under the row it belongs to.
import { Fragment, useState, type ReactNode } from "react";
import { formatCount } from "./StatTile";
import "./charts.css";

export type RankTableRow = Readonly<{
  key: string;
  requests: number;
  failures: number;
  tokens_total: number;
  last_seen_ms: number;
}>;

function shortTime(ms: number): string {
  const date = new Date(ms);
  return `${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

export function RankTable({
  rows,
  keyLabel,
  action,
  detail,
}: Readonly<{
  rows: readonly RankTableRow[];
  keyLabel: string;
  action?: (row: RankTableRow) => ReactNode;
  /** Supplying this makes rows expandable; the panel is rendered lazily, so a
   *  closed row costs nothing (it may itself issue a query). */
  detail?: (row: RankTableRow) => ReactNode;
}>) {
  const [open, setOpen] = useState<string | null>(null);
  const total = rows.reduce((sum, row) => sum + row.requests, 0);
  const columns = 7 + (action !== undefined ? 1 : 0) + (detail !== undefined ? 1 : 0);
  return (
    <table>
      <thead>
        <tr>
          {detail !== undefined ? (
            <th>
              <span className="visually-hidden">展开</span>
            </th>
          ) : null}
          <th>#</th>
          <th>{keyLabel}</th>
          <th>请求</th>
          <th>占比</th>
          <th>Token</th>
          <th>失败率</th>
          <th>最后活跃</th>
          {action !== undefined ? <th>下钻</th> : null}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => {
          const share = total > 0 ? row.requests / total : 0;
          const rate = row.requests > 0 ? row.failures / row.requests : 0;
          const expanded = open === row.key;
          return (
            <Fragment key={row.key}>
              <tr data-expanded={expanded ? "true" : undefined}>
                {detail !== undefined ? (
                  <td className="rank-toggle-cell">
                    <button
                      type="button"
                      className="rank-toggle"
                      aria-expanded={expanded}
                      aria-label={`${expanded ? "收起" : "展开"} ${row.key}`}
                      onClick={() => setOpen(expanded ? null : row.key)}
                    >
                      <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
                        <path
                          d={expanded ? "M2 4.5 6 8.5 10 4.5" : "M4.5 2 8.5 6 4.5 10"}
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="1.6"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    </button>
                  </td>
                ) : null}
                <td className="mono">{index + 1}</td>
                <td className="mono">{row.key}</td>
                <td className="mono">{formatCount(row.requests)}</td>
                <td className="share-cell">
                  <div className="share-wrap">
                    <svg
                      className="share-bar"
                      viewBox="0 0 120 8"
                      width="120"
                      height="8"
                      role="img"
                      aria-label={`占比 ${(share * 100).toFixed(1)}%`}
                    >
                      <rect className="share-track" x="0" y="0" width="120" height="8" rx="4" />
                      <rect
                        className="share-fill"
                        x="0"
                        y="0"
                        width={Math.max(share > 0 ? 3 : 0, share * 120)}
                        height="8"
                        rx="4"
                      />
                    </svg>
                    <span className="share-value mono">{(share * 100).toFixed(1)}%</span>
                  </div>
                </td>
                <td className="mono">{formatCount(row.tokens_total)}</td>
                <td className="mono">{(rate * 100).toFixed(2)}%</td>
                <td className="mono">{shortTime(row.last_seen_ms)}</td>
                {action !== undefined ? <td>{action(row)}</td> : null}
              </tr>
              {expanded && detail !== undefined ? (
                <tr className="rank-detail-row">
                  <td colSpan={columns}>{detail(row)}</td>
                </tr>
              ) : null}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}
