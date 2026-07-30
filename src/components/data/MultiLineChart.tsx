// Entity-comparison chart: up to four series on ONE shared axis (docs/07 §7.2).
//
// Why a separate component rather than a `series[]` prop on LineChart: LineChart
// is the single-measure workhorse and carries an area fill, an end label and a
// one-line tooltip — all of which are wrong here. Four filled areas occlude each
// other, four end labels collide, and the tooltip has to read out every series at
// the cursor to be useful for comparison. The axis maths is shared through
// ./scale; nothing is duplicated except the parts that genuinely differ.
//
// Still one axis. These series are the SAME measure for different entities, so
// they are directly comparable — that is the whole point. A second scale would
// make two lines that cross mean nothing.
import { useState, type PointerEvent as ReactPointerEvent } from "react";
import { axisTicks, clamp, textWidth } from "./scale";
import { useMeasuredWidth } from "./useMeasuredWidth";
import "./charts.css";

export type Series = Readonly<{
  key: string;
  /** Display name; may be an opaque identifier, so it is never parsed. */
  label: string;
  points: ReadonlyArray<Readonly<{ t: number; v: number }>>;
}>;

const PAD_L = 52;
const PAD_R = 26;
const PAD_T = 14;
const PAD_B = 26;
const TIP_FONT = 11;
const TIP_LINE = 15;

export function MultiLineChart({
  series,
  valueLabel,
  formatValue,
  formatTime,
  ariaLabel,
}: Readonly<{
  series: readonly Series[];
  valueLabel: string;
  formatValue: (value: number) => string;
  formatTime: (ms: number) => string;
  ariaLabel: string;
}>) {
  const [cursor, setCursor] = useState<number | null>(null);
  const [boxRef, width] = useMeasuredWidth(900);

  const height = 260;
  const plotW = width - PAD_L - PAD_R;
  const plotH = height - PAD_T - PAD_B;
  const baseline = PAD_T + plotH;

  // The longest series defines the x domain; shorter ones simply stop. Buckets
  // are aligned by index because every series came from the same window and
  // bucket size — the one query parameter they share.
  const length = series.reduce((best, one) => Math.max(best, one.points.length), 0);
  const lastIndex = length - 1;
  const max = series.reduce(
    (best, one) => one.points.reduce((inner, point) => (point.v > inner ? point.v : inner), best),
    0,
  );
  const ticks = axisTicks(max, 4);
  const top = ticks[ticks.length - 1] ?? 1;

  const xOf = (index: number): number =>
    lastIndex <= 0 ? PAD_L + plotW / 2 : PAD_L + (index / lastIndex) * plotW;
  const yOf = (value: number): number => baseline - (top > 0 ? value / top : 0) * plotH;

  const labelEvery = Math.max(1, Math.ceil(length / Math.max(2, Math.floor(plotW / 96))));
  const activeIndex = cursor === null ? null : clamp(cursor, 0, Math.max(0, lastIndex));
  const times = series.find((one) => one.points.length === length)?.points ?? [];

  function moveCursor(event: ReactPointerEvent<SVGSVGElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width === 0 || lastIndex <= 0) return;
    const svgX = ((event.clientX - rect.left) / rect.width) * width;
    setCursor(clamp(Math.round(((svgX - PAD_L) / plotW) * lastIndex), 0, lastIndex));
  }

  if (series.length === 0 || length === 0) {
    return <p className="stat-sub">范围内无数据</p>;
  }

  const tipLines =
    activeIndex === null
      ? []
      : [
          formatTime(times[activeIndex]?.t ?? 0),
          ...series.map(
            (one) => `${one.label} ${formatValue(one.points[activeIndex]?.v ?? 0)}`,
          ),
        ];

  return (
    <div className="chart-box" ref={boxRef}>
      <svg
        className="chart-svg"
        viewBox={`0 0 ${width} ${height}`}
        width={width}
        height={height}
        role="img"
        tabIndex={0}
        aria-label={`${ariaLabel};${series.length} 条序列,峰值 ${formatValue(max)}`}
        onPointerMove={moveCursor}
        onPointerLeave={() => setCursor(null)}
        onBlur={() => setCursor(null)}
        onKeyDown={(event) => {
          if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
            event.preventDefault();
            const step = event.key === "ArrowRight" ? 1 : -1;
            setCursor((previous) => clamp((previous ?? 0) + step, 0, lastIndex));
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

        {times.map((point, index) =>
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

        {/* No area fill: four translucent areas would occlude one another and the
            reader could no longer tell which line is on top. Line weight plus
            dash pattern carry identity alongside hue, so the series stay
            distinguishable when hue is unavailable (see charts.css). */}
        {series.map((one, index) => (
          <polyline
            key={one.key}
            className="chart-line"
            data-series={index + 1}
            points={one.points
              .map((point, i) => `${xOf(i).toFixed(1)},${yOf(point.v).toFixed(1)}`)
              .join(" ")}
          />
        ))}

        {activeIndex !== null ? (
          <>
            <line
              className="chart-crosshair"
              x1={xOf(activeIndex)}
              x2={xOf(activeIndex)}
              y1={PAD_T}
              y2={baseline}
            />
            {series.map((one, index) =>
              one.points[activeIndex] === undefined ? null : (
                <circle
                  key={one.key}
                  className="chart-dot"
                  data-series={index + 1}
                  cx={xOf(activeIndex)}
                  cy={yOf(one.points[activeIndex].v)}
                  r="3.5"
                />
              ),
            )}
            <Tooltip
              width={width}
              x={xOf(activeIndex)}
              baseline={baseline}
              lines={tipLines}
            />
          </>
        ) : null}
      </svg>
      <p className="chart-note">{valueLabel} · 全部序列共用一根纵轴,可直接比较。</p>
    </div>
  );
}

/** Anchored to the plot top rather than to a point: with four series there is no
 *  single y worth following, and a box that chases one line reads as if that
 *  line were the subject. */
function Tooltip({
  width,
  x,
  baseline,
  lines,
}: Readonly<{ width: number; x: number; baseline: number; lines: readonly string[] }>) {
  const boxW = Math.max(...lines.map((text) => textWidth(text, TIP_FONT))) + 18;
  const boxH = lines.length * TIP_LINE + 8;
  const boxX = x + 12 + boxW > width - 2 ? x - 12 - boxW : x + 12;
  const boxY = clamp(PAD_T + 4, 2, Math.max(2, baseline - boxH));
  return (
    <g>
      <rect className="chart-tip-bg" x={boxX} y={boxY} width={boxW} height={boxH} rx="7" />
      {lines.map((text, index) => (
        <text
          key={`${index}-${text}`}
          className="chart-tip-text"
          data-role={index === 0 ? "meta" : "value"}
          data-series={index === 0 ? undefined : index}
          x={boxX + 9}
          y={boxY + 16 + index * TIP_LINE}
        >
          {text}
        </text>
      ))}
    </g>
  );
}
