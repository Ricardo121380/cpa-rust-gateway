// Time-range URL contract (docs/07 §6): every observability page reads and
// writes ?range=&from=&to=&bucket=, so any "view details" action is a link.
export type RangePreset = "today" | "24h" | "7d" | "30d" | "custom";
export type Bucket = "auto" | "hour" | "day";

export type TimeRange = Readonly<{
  preset: RangePreset;
  from_ms: number;
  to_ms: number;
  bucket: Bucket;
}>;

export function resolvePreset(preset: Exclude<RangePreset, "custom">, nowMs: number): TimeRange {
  const DAY = 86_400_000;
  if (preset === "today") {
    const start = new Date(nowMs);
    start.setHours(0, 0, 0, 0);
    return { preset, from_ms: start.getTime(), to_ms: nowMs, bucket: "auto" };
  }
  const span = preset === "24h" ? DAY : preset === "7d" ? 7 * DAY : 30 * DAY;
  return { preset, from_ms: nowMs - span, to_ms: nowMs, bucket: "auto" };
}

export function rangeToParams(range: TimeRange): Record<string, string> {
  if (range.preset !== "custom") {
    return range.bucket === "auto"
      ? { range: range.preset }
      : { range: range.preset, bucket: range.bucket };
  }
  return {
    range: "custom",
    from: String(range.from_ms),
    to: String(range.to_ms),
    ...(range.bucket === "auto" ? {} : { bucket: range.bucket }),
  };
}

export function paramsToRange(params: URLSearchParams, nowMs: number): TimeRange {
  const preset = params.get("range") ?? "today";
  const bucket = (params.get("bucket") ?? "auto") as Bucket;
  if (preset === "custom") {
    const from = Number(params.get("from"));
    const to = Number(params.get("to"));
    if (Number.isFinite(from) && Number.isFinite(to) && from < to) {
      return { preset: "custom", from_ms: from, to_ms: to, bucket };
    }
    return { ...resolvePreset("today", nowMs), bucket };
  }
  const known: ReadonlyArray<Exclude<RangePreset, "custom">> = ["today", "24h", "7d", "30d"];
  const matched = known.find((candidate) => candidate === preset) ?? "today";
  return { ...resolvePreset(matched, nowMs), bucket };
}

export function resolveBucket(range: TimeRange): "hour" | "day" {
  if (range.bucket !== "auto") {
    return range.bucket;
  }
  return range.to_ms - range.from_ms <= 48 * 3_600_000 ? "hour" : "day";
}
