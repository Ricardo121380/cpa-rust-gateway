// Single-measure line chart, authored SVG (no chart library — dynamic geometry
// through presentation attributes keeps `style-src 'self'` intact).
//
// ONE Y AXIS, ALWAYS. Two measures are never overlaid on two scales; the caller
// renders small multiples instead (see the 总览 tab).
//
// Ships with the hover layer by default: a crosshair, a ringed dot and an
// in-SVG tooltip. Arrow keys walk the same cursor so the values are reachable
// without a pointer, and every caller pairs the chart with a table view.
import { useState, type PointerEvent as ReactPointerEvent } from "react";
import { axisTicks, clamp, textWidth } from "./scale";
import { useMeasuredWidth } from "./useMeasuredWidth";
import "./charts.css";

export type LinePoint = Readonly<{ t: number; v: number }>;

const PAD_L = 52;
const PAD_R = 26;
const PAD_T = 14;
const TIP_FONT = 11;
const TIP_LINE = 15;

export function LineChart({
  points,
  valueLabel,
  formatValue,
  formatTime,
  ariaLabel,
  compact = false,
  selected = null,
  onSelect,
}: Readonly<{
  points: readonly LinePoint[];
  valueLabel: string;
  formatValue: (value: number) => string;
  formatTime: (ms: number) => string;
  ariaLabel: string;
  compact?: boolean;
  /** Index into `points` to mark with a dashed rule (docs/07 §7.2). */
  selected?: number | null;
  /** Omit to make the chart read-only; supplying it makes points clickable. */
  onSelect?: (index: number) => void;
}>) {
  const [cursor, setCursor] = useState<number | null>(null);
  const [boxRef, width] = useMeasuredWidth(compact ? 460 : 900);

  const height = compact ? 150 : 250;
  const padB = compact ? 22 : 26;
  const plotW = width - PAD_L - PAD_R;
  const plotH = height - PAD_T - padB;
  const baseline = PAD_T + plotH;

  const max = points.reduce((best, point) => (point.v > best ? point.v : best), 0);
  const ticks = axisTicks(max, compact ? 3 : 4);
  const top = ticks[ticks.length - 1] ?? 1;
  const lastIndex = points.length - 1;

  const xOf = (index: number): number =>
    lastIndex <= 0 ? PAD_L + plotW / 2 : PAD_L + (index / lastIndex) * plotW;
  const yOf = (value: number): number => baseline - (top > 0 ? value / top : 0) * plotH;

  const line = points.map((point, index) => `${xOf(index).toFixed(1)},${yOf(point.v).toFixed(1)}`);
  const area = [
    `${xOf(0).toFixed(1)},${baseline}`,
    ...line,
    `${xOf(Math.max(0, lastIndex)).toFixed(1)},${baseline}`,
  ];

  const labelEvery = Math.max(1, Math.ceil(points.length / Math.max(2, Math.floor(plotW / 96))));
  const activeIndex = cursor === null ? null : clamp(cursor, 0, Math.max(0, lastIndex));
  const active = activeIndex === null ? undefined : points[activeIndex];
  const lastPoint = points[Math.max(0, lastIndex)];
  const selectedPoint =
    selected === null || selected < 0 || selected > lastIndex ? undefined : points[selected];

  function moveCursor(event: ReactPointerEvent<SVGSVGElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width === 0 || lastIndex <= 0) return;
    const svgX = ((event.clientX - rect.left) / rect.width) * width;
    const ratio = (svgX - PAD_L) / plotW;
    setCursor(clamp(Math.round(ratio * lastIndex), 0, lastIndex));
  }

  return (
    <div className="chart-box" ref={boxRef}>
      {points.length === 0 || lastPoint === undefined ? (
        <p className="stat-sub">范围内无数据</p>
      ) : (
        <svg
          className="chart-svg"
          viewBox={`0 0 ${width} ${height}`}
          width={width}
          height={height}
          role="img"
          tabIndex={0}
          aria-label={`${ariaLabel};峰值 ${formatValue(max)}${
            selectedPoint === undefined
              ? ""
              : `;已选 ${formatTime(selectedPoint.t)} ${formatValue(selectedPoint.v)}`
          }`}
          onPointerMove={moveCursor}
          onPointerLeave={() => setCursor(null)}
          onBlur={() => setCursor(null)}
          onClick={
            onSelect === undefined
              ? undefined
              : (event) => {
                  const rect = event.currentTarget.getBoundingClientRect();
                  if (rect.width === 0) return;
                  const svgX = ((event.clientX - rect.left) / rect.width) * width;
                  const ratio = lastIndex <= 0 ? 0 : (svgX - PAD_L) / plotW;
                  onSelect(clamp(Math.round(ratio * lastIndex), 0, lastIndex));
                }
          }
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault();
              const step = event.key === "ArrowRight" ? 1 : -1;
              setCursor((previous) => clamp((previous ?? 0) + step, 0, lastIndex));
            }
            // Keyboard parity: the cursor can be walked with the arrows, so it
            // must also be committable without a pointer.
            if ((event.key === "Enter" || event.key === " ") && onSelect !== undefined) {
              event.preventDefault();
              onSelect(clamp(cursor ?? 0, 0, lastIndex));
            }
            if (event.key === "Escape") setCursor(null);
          }}
        >
          {ticks.map((tick) => {
            const y = yOf(tick);
            return (
              <g key={tick}>
                <line className="chart-grid" x1={PAD_L} x2={width - PAD_R} y1={y} y2={y} />
                <text className="chart-axis-text" data-anchor="end" x={PAD_L - 8} y={y + 3.5}>
                  {formatValue(tick)}
                </text>
              </g>
            );
          })}

          {points.map((point, index) =>
            index % labelEvery === 0 ? (
              <text
                key={point.t}
                className="chart-axis-text"
                data-anchor="middle"
                x={xOf(index)}
                y={height - 7}
              >
                {formatTime(point.t)}
              </text>
            ) : null,
          )}

          <polygon className="chart-area" points={area.join(" ")} />
          <polyline className="chart-line" points={line.join(" ")} />

          {/* Selected bucket: a dashed rule plus a hollow ring. Drawn under the
              hover layer so a pointer never hides the committed selection, and
              distinguishable from the crosshair (which is transient) by the ring
              rather than by dash pattern alone. */}
          {selectedPoint !== undefined && selected !== null ? (
            <g>
              <line
                className="chart-selected"
                x1={xOf(selected)}
                x2={xOf(selected)}
                y1={PAD_T}
                y2={baseline}
              />
              <circle
                className="chart-selected-dot"
                cx={xOf(selected)}
                cy={yOf(selectedPoint.v)}
                r="5.5"
              />
            </g>
          ) : null}

          {/* selective direct label: the endpoint only, never a number per point */}
          {!compact ? (
            <text
              className="chart-endlabel"
              x={width - 4}
              y={clamp(yOf(lastPoint.v) - 10, PAD_T + 10, baseline - 4)}
            >
              {formatValue(lastPoint.v)}
            </text>
          ) : null}
          <circle className="chart-dot" cx={xOf(lastIndex)} cy={yOf(lastPoint.v)} r="4" />

          {active !== undefined && activeIndex !== null ? (
            <Tooltip
              width={width}
              x={xOf(activeIndex)}
              y={yOf(active.v)}
              baseline={baseline}
              lines={[formatTime(active.t), `${valueLabel} ${formatValue(active.v)}`]}
            />
          ) : null}
        </svg>
      )}
    </div>
  );
}

function Tooltip({
  width,
  x,
  y,
  baseline,
  lines,
}: Readonly<{
  width: number;
  x: number;
  y: number;
  baseline: number;
  lines: readonly string[];
}>) {
  const boxW = Math.max(...lines.map((text) => textWidth(text, TIP_FONT))) + 18;
  const boxH = lines.length * TIP_LINE + 8;
  const boxX = x + 12 + boxW > width - 2 ? x - 12 - boxW : x + 12;
  const boxY = clamp(y - boxH - 8, 2, baseline - boxH);
  return (
    <g>
      <line className="chart-crosshair" x1={x} x2={x} y1={PAD_T} y2={baseline} />
      <circle className="chart-dot" cx={x} cy={y} r="4.5" />
      <rect className="chart-tip-bg" x={boxX} y={boxY} width={boxW} height={boxH} rx="7" />
      {lines.map((text, index) => (
        <text
          key={`${index}-${text}`}
          className="chart-tip-text"
          data-role={index === 0 ? "meta" : "value"}
          x={boxX + 9}
          y={boxY + 16 + index * TIP_LINE}
        >
          {text}
        </text>
      ))}
    </g>
  );
}
