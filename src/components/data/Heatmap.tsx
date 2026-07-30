// weekday × hour heatmap. Sequential SINGLE HUE, lightness ramp only — never a
// rainbow, and the status pool is never borrowed for magnitude. Step 0 is the
// bare surface: "no traffic" must not read as "a little traffic".
//
// Cells are focusable buttons: click (or Enter/Space) reveals the cell's own
// window, which is what the deep link into 请求监控 is built from.
import { clamp } from "./scale";
import { useMeasuredWidth } from "./useMeasuredWidth";
import "./charts.css";

export type HeatCell = Readonly<{ weekday: number; hour: number; value: number }>;
export type HeatSelection = Readonly<{ weekday: number; hour: number }>;

const LABEL_W = 40;
const HEAD_H = 16;
const GAP = 2;

export function Heatmap({
  cells,
  weekdayLabels,
  stepOf,
  formatValue,
  metricLabel,
  selected,
  onSelect,
}: Readonly<{
  cells: readonly HeatCell[];
  weekdayLabels: readonly string[];
  stepOf: (value: number) => number;
  formatValue: (value: number) => string;
  metricLabel: string;
  selected: HeatSelection | null;
  onSelect: (cell: HeatSelection) => void;
}>) {
  const [boxRef, boxWidth] = useMeasuredWidth(880, 420);

  const cellW = clamp(Math.floor((boxWidth - LABEL_W - 24 * GAP) / 24), 14, 44);
  const cellH = clamp(Math.round(cellW * 0.6), 16, 28);
  const width = LABEL_W + 24 * (cellW + GAP);
  const height = HEAD_H + 7 * (cellH + GAP);

  const byKey = new Map(cells.map((cell) => [`${cell.weekday}-${cell.hour}`, cell.value]));
  const peak = cells.reduce((best, cell) => (cell.value > best ? cell.value : best), 0);

  return (
    <div className="chart-box" ref={boxRef}>
      <svg
        className="chart-svg"
        viewBox={`0 0 ${width} ${height}`}
        width={width}
        height={height}
        role="img"
        aria-label={`${metricLabel} 的星期 × 小时热力图,峰值 ${formatValue(peak)}`}
      >
        {Array.from({ length: 24 }, (_, hour) =>
          hour % 3 === 0 ? (
            <text
              key={`h-${hour}`}
              className="chart-axis-text"
              data-anchor="middle"
              x={LABEL_W + hour * (cellW + GAP) + cellW / 2}
              y={11}
            >
              {hour}
            </text>
          ) : null,
        )}

        {weekdayLabels.map((label, weekday) => (
          <text
            key={label}
            className="chart-axis-text"
            data-anchor="end"
            x={LABEL_W - 8}
            y={HEAD_H + weekday * (cellH + GAP) + cellH / 2 + 3.5}
          >
            {label}
          </text>
        ))}

        {weekdayLabels.map((label, weekday) =>
          Array.from({ length: 24 }, (_, hour) => {
            const value = byKey.get(`${weekday}-${hour}`) ?? 0;
            const isSelected =
              selected !== null && selected.weekday === weekday && selected.hour === hour;
            const description = `${label} ${String(hour).padStart(2, "0")}:00 · ${metricLabel} ${formatValue(value)}`;
            return (
              <rect
                key={`${weekday}-${hour}`}
                className="heat-cell"
                data-step={stepOf(value)}
                data-selected={isSelected ? "true" : undefined}
                x={LABEL_W + hour * (cellW + GAP)}
                y={HEAD_H + weekday * (cellH + GAP)}
                width={cellW}
                height={cellH}
                rx="3"
                tabIndex={0}
                role="button"
                aria-pressed={isSelected}
                aria-label={description}
                onClick={() => onSelect({ weekday, hour })}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect({ weekday, hour });
                  }
                }}
              >
                <title>{description}</title>
              </rect>
            );
          }),
        )}
      </svg>
    </div>
  );
}

export function HeatLegend({
  bins,
  formatValue,
}: Readonly<{ bins: readonly number[]; formatValue: (value: number) => string }>) {
  return (
    <div className="heat-legend">
      <span>无流量</span>
      <span className="heat-legend-swatches">
        <span className="heat-swatch" data-step="0" />
        {bins.map((bound, index) => (
          <span key={bound} className="heat-swatch" data-step={index + 1} />
        ))}
      </span>
      <span className="mono">0 → {formatValue(bins[bins.length - 1] ?? 0)}</span>
    </div>
  );
}
