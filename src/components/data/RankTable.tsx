// Rank table with share bars. The bar is MAGNITUDE, so it is one hue for every
// row — colour must never encode rank, otherwise filtering the list repaints
// the survivors and the reader re-learns the chart.
import type { ReactNode } from "react";
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
}: Readonly<{
  rows: readonly RankTableRow[];
  keyLabel: string;
  action?: (row: RankTableRow) => ReactNode;
}>) {
  const total = rows.reduce((sum, row) => sum + row.requests, 0);
  return (
    <table>
      <thead>
        <tr>
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
          return (
            <tr key={row.key}>
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
          );
        })}
      </tbody>
    </table>
  );
}
