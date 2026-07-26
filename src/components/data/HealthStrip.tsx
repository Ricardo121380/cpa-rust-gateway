// 10-minute health strip. Status pool colors + non-color redundancy:
// bad cells carry a "!" glyph, warn cells a center dot, empty are outlined —
// state is never color-alone. Each cell has a title tooltip.
import type { HealthStripState } from "../../api/proposed-types";

function timeLabel(ms: number): string {
  const date = new Date(ms);
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

const STATE_TEXT: Record<HealthStripState, string> = {
  empty: "无请求",
  ok: "正常",
  warn: "有告警",
  bad: "有失败",
};

export function HealthStrip({
  buckets,
}: Readonly<{ buckets: ReadonlyArray<{ bucket_start_ms: number; state: HealthStripState }> }>) {
  return (
    <div
      className="health-strip"
      role="img"
      aria-label={`请求健康条带,${buckets.length} 个 10 分钟桶`}
    >
      {buckets.map((bucket) => (
        <span
          key={bucket.bucket_start_ms}
          className="health-cell"
          data-state={bucket.state}
          title={`${timeLabel(bucket.bucket_start_ms)} · ${STATE_TEXT[bucket.state]}`}
        >
          {bucket.state === "bad" ? "!" : bucket.state === "warn" ? "·" : ""}
        </span>
      ))}
    </div>
  );
}
