// Legend for MultiLineChart. The swatch is a real line sample carrying the same
// dash pattern as the series it names, so the mapping survives the cases where
// hue does not: forced-colors, greyscale, colour-vision deficiency.
import "./charts.css";

export function SeriesLegend({
  items,
}: Readonly<{ items: ReadonlyArray<Readonly<{ key: string; label: string }>> }>) {
  return (
    <ul className="chart-legend">
      {items.map((item, index) => (
        <li key={item.key}>
          <svg width="22" height="10" aria-hidden="true" focusable="false">
            <line
              className="chart-line"
              data-series={index + 1}
              x1="1"
              y1="5"
              x2="21"
              y2="5"
            />
          </svg>
          <span className="mono">{item.label}</span>
        </li>
      ))}
    </ul>
  );
}
