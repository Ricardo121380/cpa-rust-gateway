// Compact request-volume timeline (MVP; the full ECharts trend with dataZoom
// arrives in FE-4). Bars = requests in chart-1; a failure cap in the critical
// status color sits on top of each bar (with tooltip text redundancy).
import { formatCount } from "./StatTile";

export function MiniTimeline({
  buckets,
}: Readonly<{
  buckets: ReadonlyArray<{ bucket_start_ms: number; requests: number; failures: number }>;
}>) {
  if (buckets.length === 0) {
    return <p className="stat-sub">范围内无数据</p>;
  }
  const width = 640;
  const height = 120;
  const gap = 2;
  const barWidth = Math.max(2, width / buckets.length - gap);
  const max = Math.max(...buckets.map((bucket) => bucket.requests), 1);
  return (
    <svg
      className="mini-timeline"
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={`流量趋势,${buckets.length} 个桶,峰值 ${formatCount(max)} 请求`}
    >
      {buckets.map((bucket, index) => {
        const x = index * (barWidth + gap);
        const barHeight = Math.max(1, (bucket.requests / max) * (height - 6));
        const failHeight = bucket.requests > 0
          ? Math.min(barHeight, (bucket.failures / bucket.requests) * barHeight)
          : 0;
        const label = `${new Date(bucket.bucket_start_ms).toLocaleString()} · ${bucket.requests} 请求 / ${bucket.failures} 失败`;
        return (
          <g key={bucket.bucket_start_ms}>
            <title>{label}</title>
            <rect
              x={x}
              y={height - barHeight}
              width={barWidth}
              height={barHeight}
              rx="1.5"
              fill="var(--chart-1)"
              opacity="0.85"
            />
            {failHeight > 0.5 ? (
              <rect
                x={x}
                y={height - barHeight}
                width={barWidth}
                height={failHeight}
                rx="1.5"
                fill="var(--status-critical)"
              />
            ) : null}
          </g>
        );
      })}
    </svg>
  );
}
