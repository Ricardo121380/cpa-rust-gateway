import { describe, expect, it } from "vitest";
import {
  axisTicks,
  buildFilters,
  cellParam,
  cellWindow,
  parseCell,
  failureRate,
  formatMetric,
  hasActiveFilter,
  heatBins,
  heatStep,
  includeForTab,
  metricValue,
  monitoringHref,
  niceCeil,
  parseMetric,
  parseStatus,
  parseTab,
  rankTotal,
  shareOf,
  COMPARE_LIMIT,
  compareColorIndex,
  compareFilters,
  compareKeys,
  applyZoom,
  findBucketIndex,
  parseSelectedBucket,
  parseZoom,
  zoomAvailable,
  zoomParam,
  ZOOM_THRESHOLD,
  type RankRow,
  type TimelineBucket,
} from "./model";

const HOUR = 3_600_000;
const NOW = 1785060000000;

describe("url contract", () => {
  it("tab falls back to overview for unknown / missing values", () => {
    expect(parseTab("heatmap")).toBe("heatmap");
    expect(parseTab("credentials")).toBe("credentials");
    expect(parseTab("cost")).toBe("overview");
    expect(parseTab(null)).toBe("overview");
  });

  it("metric and status are closed enums", () => {
    expect(parseMetric("tokens")).toBe("tokens");
    expect(parseMetric("dollars")).toBe("requests");
    expect(parseStatus("failed")).toBe("failed");
    expect(parseStatus("weird")).toBe("all");
  });

  it("filters carry only closed enums and returned identifiers", () => {
    expect(buildFilters("all", null)).toEqual({ status: "all" });
    expect(buildFilters("failed", "glm-5-air")).toEqual({
      status: "failed",
      public_model: ["glm-5-air"],
    });
    expect(hasActiveFilter("all", null)).toBe(false);
    expect(hasActiveFilter("all", "")).toBe(false);
    expect(hasActiveFilter("all", "glm-5-air")).toBe(true);
    expect(hasActiveFilter("failed", null)).toBe(true);
  });
});

describe("per-tab include projection", () => {
  it("asks for exactly what the tab renders", () => {
    expect(includeForTab("overview", "requests")).toEqual({
      options: true,
      summary: true,
      timeline: true,
      ranks: { by: "public_model", limit: 8 },
    });
    expect(includeForTab("trend", "tokens")).toEqual({ options: true, timeline: true });
    expect(includeForTab("models", "requests")).toEqual({
      options: true,
      ranks: { by: "public_model", limit: 20 },
    });
    expect(includeForTab("credentials", "requests")).toEqual({
      options: true,
      ranks: { by: "credential", limit: 20 },
    });
  });

  it("heatmap carries the selected metric", () => {
    expect(includeForTab("heatmap", "failure_rate")).toEqual({
      options: true,
      heatmap: { metric: "failure_rate" },
    });
  });

  it("never requests events (that is 请求监控's job)", () => {
    for (const tab of ["overview", "trend", "models", "credentials", "heatmap"] as const) {
      expect(includeForTab(tab, "requests")).not.toHaveProperty("events");
    }
  });
});

describe("timeline metrics", () => {
  const bucket: TimelineBucket = {
    bucket_start_ms: NOW,
    requests: 200,
    failures: 4,
    tokens_total: 1_800_000,
  };

  it("reads one measure per metric", () => {
    expect(metricValue(bucket, "requests")).toBe(200);
    expect(metricValue(bucket, "tokens")).toBe(1_800_000);
    expect(metricValue(bucket, "failure_rate")).toBeCloseTo(0.02);
  });

  it("failure rate of an empty bucket is 0, not NaN", () => {
    expect(metricValue({ ...bucket, requests: 0, failures: 0 }, "failure_rate")).toBe(0);
  });

  it("formats per metric", () => {
    expect(formatMetric(0.02, "failure_rate")).toBe("2.00%");
    expect(formatMetric(1_800_000, "tokens")).toBe("1.8M");
    expect(formatMetric(200, "requests")).toBe("200");
  });
});

describe("axis scale", () => {
  it("rounds up to clean tops", () => {
    expect(niceCeil(0)).toBe(1);
    expect(niceCeil(7)).toBe(10);
    expect(niceCeil(180)).toBe(200);
    expect(niceCeil(1)).toBe(1);
    expect(niceCeil(2.4)).toBe(2.5);
  });

  it("ticks span 0..top inclusive", () => {
    const ticks = axisTicks(180, 4);
    expect(ticks).toEqual([0, 50, 100, 150, 200]);
  });
});

describe("heatmap ramp", () => {
  it("zero never borrows a colour step", () => {
    expect(heatStep(0, 200)).toBe(0);
    expect(heatStep(5, 0)).toBe(0);
  });

  it("steps rise monotonically and stop at the top step", () => {
    expect(heatStep(1, 200)).toBe(1);
    expect(heatStep(200, 200)).toBe(5);
    let previous = 0;
    for (let value = 0; value <= 200; value += 10) {
      const step = heatStep(value, 200);
      expect(step).toBeGreaterThanOrEqual(previous);
      previous = step;
    }
  });

  it("legend bins cover the range", () => {
    const bins = heatBins(200);
    expect(bins).toHaveLength(5);
    expect(bins[bins.length - 1]).toBe(200);
  });
});

describe("selected cell in the url", () => {
  it("round-trips a valid cell", () => {
    expect(parseCell(cellParam({ weekday: 3, hour: 14 }))).toEqual({ weekday: 3, hour: 14 });
    expect(parseCell("0-0")).toEqual({ weekday: 0, hour: 0 });
    expect(parseCell("6-23")).toEqual({ weekday: 6, hour: 23 });
  });

  it("rejects out-of-range and malformed values", () => {
    expect(parseCell(null)).toBeNull();
    expect(parseCell("7-1")).toBeNull();
    expect(parseCell("1-24")).toBeNull();
    expect(parseCell("1")).toBeNull();
    expect(parseCell("a-b")).toBeNull();
  });
});

describe("cell → time window", () => {
  it("returns the most recent local occurrence inside the range", () => {
    const to = NOW;
    const from = to - 7 * 24 * HOUR;
    const probe = new Date(to - 50 * HOUR);
    probe.setMinutes(0, 0, 0);
    const window = cellWindow(probe.getDay(), probe.getHours(), from, to);
    expect(window).not.toBeNull();
    const found = new Date(window!.from_ms);
    expect(found.getDay()).toBe(probe.getDay());
    expect(found.getHours()).toBe(probe.getHours());
    expect(window!.from_ms).toBeGreaterThanOrEqual(from);
    expect(window!.to_ms).toBeLessThanOrEqual(to);
    // most recent: a later matching hour would be within one week of `to`
    expect(to - window!.from_ms).toBeLessThan(7 * 24 * HOUR);
  });

  it("clamps the window's end to the range end", () => {
    const to = NOW;
    const cursor = new Date(to);
    cursor.setMinutes(0, 0, 0);
    const window = cellWindow(cursor.getDay(), cursor.getHours(), to - 24 * HOUR, to);
    expect(window).not.toBeNull();
    expect(window!.to_ms).toBe(to);
  });

  it("returns null when the range does not cover the cell", () => {
    const to = NOW;
    const from = to - 2 * HOUR;
    const cursor = new Date(to - 40 * HOUR);
    expect(cellWindow(cursor.getDay(), cursor.getHours(), from, to)).toBeNull();
  });
});

describe("deep link", () => {
  it("encodes the window as a custom range plus the live filters", () => {
    const href = monitoringHref({ from_ms: 1000, to_ms: 2000 }, "failed", "glm-5-air");
    const params = new URLSearchParams(href.slice(href.indexOf("?")));
    expect(href.startsWith("/monitoring?")).toBe(true);
    expect(params.get("range")).toBe("custom");
    expect(params.get("from")).toBe("1000");
    expect(params.get("to")).toBe("2000");
    expect(params.get("bucket")).toBe("hour");
    expect(params.get("status")).toBe("failed");
    expect(params.get("model")).toBe("glm-5-air");
  });

  it("omits inactive filters", () => {
    const href = monitoringHref({ from_ms: 1000, to_ms: 2000 }, "all", null);
    const params = new URLSearchParams(href.slice(href.indexOf("?")));
    expect(params.get("status")).toBeNull();
    expect(params.get("model")).toBeNull();
  });
});

describe("rank rows", () => {
  const rows: readonly RankRow[] = [
    { key: "minimax-m3", requests: 720, failures: 18, tokens_total: 8_000_000, last_seen_ms: NOW },
    { key: "glm-5-air", requests: 280, failures: 0, tokens_total: 2_000_000, last_seen_ms: NOW },
  ];

  it("shares are computed against the visible total", () => {
    const total = rankTotal(rows);
    expect(total).toBe(1000);
    expect(shareOf(rows[0]!.requests, total)).toBeCloseTo(0.72);
    expect(shareOf(1, 0)).toBe(0);
  });

  it("failure rate of a silent entity is 0", () => {
    expect(failureRate(rows[0]!)).toBeCloseTo(0.025);
    expect(failureRate({ ...rows[1]!, requests: 0 })).toBe(0);
  });
});

describe("entity comparison", () => {
  const ranks: readonly RankRow[] = [
    { key: "a", requests: 500, failures: 5, tokens_total: 9, last_seen_ms: NOW },
    { key: "b", requests: 400, failures: 4, tokens_total: 8, last_seen_ms: NOW },
    { key: "c", requests: 300, failures: 3, tokens_total: 7, last_seen_ms: NOW },
    { key: "d", requests: 200, failures: 2, tokens_total: 6, last_seen_ms: NOW },
    { key: "e", requests: 100, failures: 1, tokens_total: 5, last_seen_ms: NOW },
  ];

  it("takes top-N in rank order and never exceeds the palette", () => {
    const keys = compareKeys(ranks);
    expect(keys).toEqual(["a", "b", "c", "d"]);
    expect(keys.length).toBeLessThanOrEqual(COMPARE_LIMIT);
  });

  it("tolerates a missing or short rank list", () => {
    expect(compareKeys(undefined)).toEqual([]);
    expect(compareKeys(ranks.slice(0, 2))).toEqual(["a", "b"]);
  });

  it("rank order is the colour order, so hues do not reshuffle", () => {
    const keys = compareKeys(ranks);
    expect(compareColorIndex(keys, "a")).toBe(0);
    expect(compareColorIndex(keys, "d")).toBe(3);
    // dropped out of the top N: the caller must not silently recolour it
    expect(compareColorIndex(keys, "e")).toBe(-1);
  });

  it("pins exactly one dimension per series and keeps the shared filters", () => {
    const base = buildFilters("failed", "glm-5-air");
    expect(compareFilters(base, "clientKeys", "key-ci")).toEqual({
      status: "failed",
      public_model: ["glm-5-air"],
      client_key_id: ["key-ci"],
    });
    expect(compareFilters(base, "credentials", "cred-x")).toEqual({
      status: "failed",
      public_model: ["glm-5-air"],
      credential_id: ["cred-x"],
    });
  });

  it("a model series overrides the shared model filter rather than conflicting", () => {
    const base = buildFilters("all", "glm-5-air");
    expect(compareFilters(base, "models", "minimax-m3")).toEqual({
      status: "all",
      public_model: ["minimax-m3"],
    });
  });
});

describe("the six tabs each project their own include", () => {
  it("every rank tab asks for its own dimension", () => {
    expect(includeForTab("models", "requests").ranks?.by).toBe("public_model");
    expect(includeForTab("clientKeys", "requests").ranks?.by).toBe("client_key");
    expect(includeForTab("credentials", "requests").ranks?.by).toBe("credential");
  });

  it("no tab asks for a projection it does not render", () => {
    expect(includeForTab("clientKeys", "requests").heatmap).toBeUndefined();
    expect(includeForTab("clientKeys", "requests").timeline).toBeUndefined();
    expect(includeForTab("trend", "requests").ranks).toBeUndefined();
  });

  it("clientKeys is a real tab, not a fallback to overview", () => {
    expect(parseTab("clientKeys")).toBe("clientKeys");
    expect(parseTab("nope")).toBe("overview");
  });
});

describe("zoom window", () => {
  it("only offered above the threshold", () => {
    expect(zoomAvailable(ZOOM_THRESHOLD)).toBe(false);
    expect(zoomAvailable(ZOOM_THRESHOLD + 1)).toBe(true);
    expect(zoomAvailable(0)).toBe(false);
  });

  it("round-trips through the URL", () => {
    const window = parseZoom("3-9", 24);
    expect(window).toEqual({ start: 3, end: 9 });
    expect(zoomParam(window)).toBe("3-9");
    expect(zoomParam(null)).toBeNull();
  });

  it("clamps a stale link whose window has since shrunk", () => {
    // shared link said 10-40 but the range now holds 12 buckets
    expect(parseZoom("10-40", 12)).toEqual({ start: 10, end: 11 });
  });

  it("orders reversed handles rather than producing an inverted window", () => {
    expect(parseZoom("9-3", 24)).toEqual({ start: 3, end: 9 });
  });

  it("treats degenerate and full-coverage windows as no zoom", () => {
    expect(parseZoom("5-5", 24)).toBeNull(); // a single bucket has no shape
    expect(parseZoom("0-23", 24)).toBeNull(); // same as unzoomed
    expect(parseZoom("garbage", 24)).toBeNull();
    expect(parseZoom(null, 24)).toBeNull();
    expect(parseZoom("0-5", 0)).toBeNull();
  });

  it("slices inclusively and is a no-op when absent", () => {
    const items = [0, 1, 2, 3, 4, 5];
    expect(applyZoom(items, { start: 1, end: 3 })).toEqual([1, 2, 3]);
    expect(applyZoom(items, null)).toEqual(items);
  });
});

describe("selected bucket", () => {
  const buckets: readonly TimelineBucket[] = [
    { bucket_start_ms: 1000, requests: 1, failures: 0, tokens_total: 0 },
    { bucket_start_ms: 2000, requests: 2, failures: 0, tokens_total: 0 },
    { bucket_start_ms: 3000, requests: 3, failures: 0, tokens_total: 0 },
  ];

  it("is stored by start time, not by index", () => {
    // an index would point at a different bucket after a zoom or bucket change
    expect(parseSelectedBucket("2000")).toBe(2000);
    expect(findBucketIndex(buckets, 2000)).toBe(1);
  });

  it("rejects nonsense and reports a bucket that is no longer visible", () => {
    expect(parseSelectedBucket("nope")).toBeNull();
    expect(parseSelectedBucket("0")).toBeNull();
    expect(parseSelectedBucket(null)).toBeNull();
    expect(findBucketIndex(buckets, 9999)).toBeNull();
    expect(findBucketIndex(buckets, null)).toBeNull();
  });

  it("survives a zoom that still contains it, and drops out of one that does not", () => {
    const zoomed = applyZoom(buckets, { start: 1, end: 2 });
    expect(findBucketIndex(zoomed, 2000)).toBe(0);
    expect(findBucketIndex(zoomed, 1000)).toBeNull();
  });
});
