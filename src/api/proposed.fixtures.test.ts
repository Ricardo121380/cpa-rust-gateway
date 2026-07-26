// Integration: proposed G3 analytics/dashboard against the fixture backend
// (vitest env enables fixtures). Verifies internal consistency the pages
// depend on: summary vs timeline, cursor paging, status filter.
import { beforeAll, describe, expect, it } from "vitest";
import { useSessionStore } from "../session/sessionStore";
import { fetchProposedAnalytics, fetchProposedDashboard, fetchProposedGraph } from "./proposed";

beforeAll(() => {
  useSessionStore.getState().unlock(`mgmt_${"a".repeat(40)}`, `csrf_${"b".repeat(40)}`);
});

const HOUR = 3_600_000;
const FROM = 1785000000000;

describe("proposed analytics fixture", () => {
  it("summary totals equal timeline sums for the same range", async () => {
    const result = await fetchProposedAnalytics({
      from_ms: FROM,
      to_ms: FROM + 24 * HOUR,
      timezone: "Asia/Shanghai",
      bucket: "hour",
      include: { summary: true, timeline: true },
    });
    expect(result.range.bucket).toBe("hour");
    expect(result.timeline).toHaveLength(24);
    const requests = (result.timeline ?? []).reduce((sum, bucket) => sum + bucket.requests, 0);
    expect(result.summary?.requests).toBe(requests);
  });

  it("events page respects cursor and terminates", async () => {
    const query = {
      from_ms: FROM,
      to_ms: FROM + 24 * HOUR,
      timezone: "Asia/Shanghai",
      bucket: "hour" as const,
      include: { events: { cursor: null as string | null, limit: 25 } },
    };
    const first = await fetchProposedAnalytics(query);
    expect(first.events?.items).toHaveLength(25);
    expect(first.events?.next_cursor).not.toBeNull();
    let cursor = first.events?.next_cursor ?? null;
    let total = first.events?.items.length ?? 0;
    let guard = 0;
    while (cursor !== null && guard < 10) {
      const page = await fetchProposedAnalytics({
        ...query,
        include: { events: { cursor, limit: 25 } },
      });
      total += page.events?.items.length ?? 0;
      cursor = page.events?.next_cursor ?? null;
      guard += 1;
    }
    expect(total).toBe(57);
  });

  it("failed filter returns only failed rows", async () => {
    const result = await fetchProposedAnalytics({
      from_ms: FROM,
      to_ms: FROM + 24 * HOUR,
      timezone: "Asia/Shanghai",
      bucket: "hour",
      filters: { status: "failed" },
      include: { events: { cursor: null, limit: 100 } },
    });
    const items = result.events?.items ?? [];
    expect(items.length).toBeGreaterThan(0);
    expect(items.every((row) => row.outcome === "failed" && row.error_code !== null)).toBe(true);
  });

  it("dashboard summary provides strip and mix", async () => {
    const summary = await fetchProposedDashboard(FROM, FROM + 8 * HOUR);
    expect(summary.health_strip.length).toBe(48); // 8h of 10-minute buckets
    expect(summary.kpi.requests).toBeGreaterThan(0);
    expect(summary.token_mix.cache_read).toBeGreaterThan(0);
  });

  it("graph slices exist for seeded upstream", async () => {
    const graph = await fetchProposedGraph("draft-2026-08");
    expect(graph.endpoints.length).toBeGreaterThan(0);
    expect(graph.credentials.some((credential) => credential.kind === "oauth")).toBe(true);
  });
});
