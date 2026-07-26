// Token composition bar: fixed categorical assignment (color follows the
// entity, never rank), 2px surface gaps between segments, legend always
// present with values in ink — per dataviz spec.
import type { TokenSummary } from "../../api/proposed-types";
import { formatCount } from "./StatTile";

const SLOTS = [
  { key: "input", label: "输入", color: "var(--chart-1)" },
  { key: "output", label: "输出", color: "var(--chart-2)" },
  { key: "reasoning", label: "推理", color: "var(--chart-3)" },
  { key: "cache_read", label: "缓存读", color: "var(--chart-4)" },
] as const;

export function TokenMixBar({ tokens }: Readonly<{ tokens: TokenSummary }>) {
  const parts = SLOTS.map((slot) => ({ ...slot, value: tokens[slot.key] ?? 0 }));
  const total = parts.reduce((sum, part) => sum + part.value, 0);
  if (total === 0) {
    return <p className="stat-sub">暂无 Token 数据</p>;
  }
  return (
    <div>
      <div className="token-mix" role="img" aria-label="Token 构成占比条">
        {parts
          .filter((part) => part.value > 0)
          .map((part) => (
            <span
              key={part.key}
              className="token-seg"
              style={{ flexGrow: part.value, background: part.color }}
              title={`${part.label} ${formatCount(part.value)}(${((part.value / total) * 100).toFixed(1)}%)`}
            />
          ))}
      </div>
      <div className="chart-legend">
        {parts.map((part) => (
          <span key={part.key} className="legend-item">
            <span className="legend-dot" style={{ background: part.color }} />
            {part.label} <span className="mono">{formatCount(part.value)}</span>
          </span>
        ))}
      </div>
    </div>
  );
}
