// Range brush for a bucketed series (docs/07 §7.2: ">12 桶时内置 dataZoom").
//
// Shows the WHOLE series as a low-contrast sparkline with the selected window
// highlighted, so the control answers "what am I cutting out of" — a pair of bare
// number inputs would not. The two handles are real range inputs: dragging an SVG
// handle is a pointer-only affordance, whereas <input type="range"> is keyboard
// operable, screen-reader labelled and touch-friendly for free.
//
// Deliberately not a generic dataZoom: it only selects a contiguous bucket window
// on one axis, which is the only zoom the analytics contract can express (the
// window is re-derived, not resampled).
import { useId } from "react";
import "./charts.css";

export function ZoomBrush({
  values,
  start,
  end,
  formatIndex,
  onChange,
  onReset,
}: Readonly<{
  /** Every bucket's value, unzoomed — the overview the brush sits on. */
  values: readonly number[];
  start: number;
  end: number;
  /** Label for a bucket index, used by the handles' accessible text. */
  formatIndex: (index: number) => string;
  onChange: (next: Readonly<{ start: number; end: number }>) => void;
  onReset: () => void;
}>) {
  const id = useId();
  const last = Math.max(0, values.length - 1);
  const max = values.reduce((best, value) => (value > best ? value : best), 0);
  const full = start === 0 && end === last;

  // Viewbox units; the element is stretched by CSS, so these are ratios.
  const W = 100;
  const H = 24;
  const xOf = (index: number): number => (last <= 0 ? 0 : (index / last) * W);
  const points = values
    .map((value, index) => `${xOf(index).toFixed(2)},${(H - (max > 0 ? value / max : 0) * H).toFixed(2)}`)
    .join(" ");

  return (
    <div className="zoom-brush">
      <svg
        className="zoom-overview"
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        aria-hidden="true"
        focusable="false"
      >
        {/* the excluded regions, dimmed rather than hidden: the reader keeps the
            shape of the whole window for context */}
        <rect className="zoom-outside" x="0" y="0" width={xOf(start)} height={H} />
        <rect
          className="zoom-outside"
          x={xOf(end)}
          y="0"
          width={Math.max(0, W - xOf(end))}
          height={H}
        />
        <polyline className="zoom-spark" points={points} />
        <rect
          className="zoom-window"
          x={xOf(start)}
          y="0.5"
          width={Math.max(0.5, xOf(end) - xOf(start))}
          height={H - 1}
        />
      </svg>

      <div className="zoom-handles">
        <label className="visually-hidden" htmlFor={`${id}-start`}>
          窗口起点
        </label>
        <input
          id={`${id}-start`}
          type="range"
          min={0}
          max={last}
          value={start}
          aria-valuetext={formatIndex(start)}
          onChange={(event) => {
            const next = Number(event.target.value);
            // Handles may not cross; a start pushed past the end drags the end
            // with it instead of producing an inverted window.
            onChange({ start: next, end: Math.max(next + 1, end) > last ? last : Math.max(next + 1, end) });
          }}
        />
        <label className="visually-hidden" htmlFor={`${id}-end`}>
          窗口终点
        </label>
        <input
          id={`${id}-end`}
          type="range"
          min={0}
          max={last}
          value={end}
          aria-valuetext={formatIndex(end)}
          onChange={(event) => {
            const next = Number(event.target.value);
            onChange({ start: Math.min(start, Math.max(0, next - 1)), end: next });
          }}
        />
      </div>

      <p className="zoom-status">
        <span className="mono">
          {formatIndex(start)} → {formatIndex(end)}
        </span>
        <span className="muted"> · {end - start + 1} / {values.length} 桶</span>
        {full ? null : (
          <button type="button" className="chip-off zoom-reset" onClick={onReset}>
            重置范围
          </button>
        )}
      </p>
    </div>
  );
}
