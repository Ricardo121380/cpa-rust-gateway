// Token composition bar. Rendered as SVG (presentation attributes, not CSS
// style attributes) so it survives a strict `style-src 'self'` CSP with no
// inline-style exemption — the browser smoke suite proved inline styles are
// blocked under our own policy.
import type { TokenSummary } from "../../api/proposed-types";
import { formatCount } from "./StatTile";

const SLOTS = [
  { key: "input", label: "输入", varName: "--chart-1" },
  { key: "output", label: "输出", varName: "--chart-2" },
  { key: "reasoning", label: "推理", varName: "--chart-3" },
  { key: "cache_read", label: "缓存读", varName: "--chart-4" },
] as const;

export function TokenMixBar({ tokens }: Readonly<{ tokens: TokenSummary }>) {
  const parts = SLOTS.map((slot) => ({ ...slot, value: tokens[slot.key] ?? 0 })).filter(
    (part) => part.value > 0,
  );
  const total = parts.reduce((sum, part) => sum + part.value, 0);
  if (total === 0) {
    return <p className="stat-sub">暂无 Token 数据</p>;
  }

  const width = 600;
  const height = 26;
  const gap = 2;
  const usable = width - gap * Math.max(0, parts.length - 1);
  let cursor = 0;

  return (
    <div>
      <svg
        className="token-mix"
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`Token 构成:${parts
          .map((part) => `${part.label} ${((part.value / total) * 100).toFixed(1)}%`)
          .join("、")}`}
      >
        {parts.map((part) => {
          const segWidth = (part.value / total) * usable;
          const x = cursor;
          cursor += segWidth + gap;
          return (
            <rect
              key={part.key}
              x={x}
              y={0}
              width={Math.max(1, segWidth)}
              height={height}
              rx="4"
              fill={`var(${part.varName})`}
            >
              <title>{`${part.label} ${formatCount(part.value)}(${((part.value / total) * 100).toFixed(1)}%)`}</title>
            </rect>
          );
        })}
      </svg>
      <div className="chart-legend">
        {SLOTS.map((slot) => (
          <span key={slot.key} className="legend-item">
            <span className={`legend-dot legend-dot-${slot.key}`} />
            {slot.label} <span className="mono">{formatCount(tokens[slot.key] ?? 0)}</span>
          </span>
        ))}
      </div>
    </div>
  );
}
