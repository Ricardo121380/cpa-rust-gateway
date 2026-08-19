import { describe, expect, it } from "vitest";
import {
  activeFilterCount,
  collect,
  confidenceTone,
  formatTokens,
  formatWatermark,
  groupBy,
  parseDimension,
  parseFilters,
  parseRange,
  rangeParams,
  shareOf,
  sumFamily,
  UNGROUPED_LABEL,
  weakest,
  type Confidence,
  type UsageResponse,
  type UsageRow,
} from "./model";

/** Row builder: everything defaults to observed/exact so each test can vary the
 *  one thing it is about. */
function row(over: Partial<UsageRow> = {}): UsageRow {
  const family = (total: number | null, confidence: Confidence = "exact") => ({
    total,
    confidence,
  });
  return {
    provider_id: "prov-a",
    channel_id: "ch-a",
    account_id: "acct-a",
    public_model: "m-1",
    protocol: "openai_responses",
    client_key_id: "ck-1",
    access_group_id: "ag-1",
    request_count: 1,
    usage_observations: 1,
    observed_at_ms: 1_700_000_000_000,
    cost_microunits: null,
    cost_confidence: "unpriced",
    input_tokens: family(100),
    output_tokens: family(20),
    reasoning_tokens: family(0),
    cache_read_tokens: family(0),
    cache_creation_tokens: family(0),
    cached_tokens: family(0),
    ...over,
  };
}

describe("weakest", () => {
  it("ranks exact < partial < unknown and returns the worst", () => {
    expect(weakest(["exact", "exact"])).toBe("exact");
    expect(weakest(["exact", "partial"])).toBe("partial");
    expect(weakest(["partial", "unknown", "exact"])).toBe("unknown");
  });

  it("treats an empty set as exact — nothing observed cannot be contradicted", () => {
    expect(weakest([])).toBe("exact");
  });
});

describe("sumFamily", () => {
  it("adds observed totals and keeps exact when every contributor is exact", () => {
    const total = sumFamily([row(), row()], "input_tokens");
    expect(total).toEqual({ total: 200, confidence: "exact", partialCoverage: false });
  });

  it("downgrades to the weakest contributor's confidence", () => {
    const total = sumFamily(
      [row(), row({ input_tokens: { total: 5, confidence: "partial" } })],
      "input_tokens",
    );
    expect(total.total).toBe(105);
    expect(total.confidence).toBe("partial");
    expect(total.partialCoverage).toBe(false);
  });

  it("NEVER counts an unobserved total as zero", () => {
    // This is the whole point of the type. A null contributor means the sum is
    // a floor; adding it as 0 would under-report while looking precise.
    const total = sumFamily(
      [row(), row({ input_tokens: { total: null, confidence: "unknown" } })],
      "input_tokens",
    );
    expect(total.total).toBe(100);
    expect(total.partialCoverage).toBe(true);
    expect(total.confidence).toBe("unknown");
  });

  it("returns null, not 0, when nothing was observed at all", () => {
    const total = sumFamily(
      [row({ input_tokens: { total: null, confidence: "unknown" } })],
      "input_tokens",
    );
    expect(total.total).toBeNull();
    expect(total.partialCoverage).toBe(true);
  });

  it("distinguishes a real zero from an absent observation", () => {
    const zero = sumFamily([row({ input_tokens: { total: 0, confidence: "exact" } })], "input_tokens");
    expect(zero.total).toBe(0);
    expect(zero.partialCoverage).toBe(false);
    expect(zero.confidence).toBe("exact");
  });

  it("sums an empty row set to null", () => {
    expect(sumFamily([], "output_tokens").total).toBeNull();
  });
});

describe("groupBy", () => {
  it("groups by one dimension and sums requests within the group", () => {
    const groups = groupBy(
      [row({ provider_id: "a" }), row({ provider_id: "a" }), row({ provider_id: "b" })],
      "provider_id",
    );
    expect(groups.map((group) => group.key)).toEqual(["a", "b"]);
    expect(groups[0]?.request_count).toBe(2);
    expect(groups[0]?.families.input_tokens.total).toBe(200);
  });

  it("orders by request_count descending, then key, so refetches are stable", () => {
    const groups = groupBy(
      [
        row({ provider_id: "b", request_count: 1 }),
        row({ provider_id: "a", request_count: 5 }),
        row({ provider_id: "c", request_count: 1 }),
      ],
      "provider_id",
    );
    expect(groups.map((group) => group.key)).toEqual(["a", "b", "c"]);
  });

  it("gives a null access group its own bucket instead of dropping it", () => {
    // "belongs to no access group" is a real fact about the deployment; folding
    // it into "" would hide Client Keys that answer to no group limits.
    const groups = groupBy(
      [row({ access_group_id: null }), row({ access_group_id: "ag-1" })],
      "access_group_id",
    );
    expect(groups.map((group) => group.key).sort()).toEqual([UNGROUPED_LABEL, "ag-1"].sort());
    expect(groups.find((group) => group.key === UNGROUPED_LABEL)?.value).toBeNull();
  });

  it("propagates partial coverage from any row in the group", () => {
    const groups = groupBy(
      [row(), row({ output_tokens: { total: null, confidence: "unknown" } })],
      "provider_id",
    );
    expect(groups[0]?.families.output_tokens.partialCoverage).toBe(true);
    expect(groups[0]?.families.input_tokens.partialCoverage).toBe(false);
  });
});

describe("collect", () => {
  const page = (over: Partial<UsageResponse> = {}): UsageResponse => ({
    observed_through_ms: 1_700_000_000_000,
    items: [row()],
    next_cursor: null,
    ...over,
  });

  it("concatenates every page's rows", () => {
    expect(collect([page(), page()]).rows).toHaveLength(2);
  });

  it("takes the watermark from the freshest read", () => {
    expect(
      collect([page(), page({ observed_through_ms: 1_700_000_999_000 })]).observed_through_ms,
    ).toBe(1_700_000_999_000);
  });

  it("is not truncated when the last page has no cursor", () => {
    expect(collect([page()]).truncated).toBe(false);
  });

  it("reports truncation when the page cap is hit with a cursor outstanding", () => {
    // The alternative is presenting a partial sum as a total — a silent cap is
    // exactly the failure this flag exists to prevent.
    const pages = Array.from({ length: 20 }, () => page({ next_cursor: "more" }));
    expect(collect(pages).truncated).toBe(true);
  });

  it("handles the empty case without inventing a watermark", () => {
    expect(collect([]).observed_through_ms).toBeNull();
    expect(collect([]).rows).toEqual([]);
  });
});

describe("filters", () => {
  it("reads only the declared keys and trims them", () => {
    const params = new Map([
      ["provider_id", " prov-a "],
      ["model", "m-1"],
      ["nonsense", "x"],
    ]);
    expect(parseFilters((key) => params.get(key) ?? null)).toEqual({
      provider_id: "prov-a",
      model: "m-1",
    });
  });

  it("drops an unknown protocol rather than forwarding it into a 400", () => {
    const params = new Map([["protocol", "grpc"]]);
    expect(parseFilters((key) => params.get(key) ?? null)).toEqual({});
  });

  it("keeps a protocol the contract declares", () => {
    const params = new Map([["protocol", "anthropic_messages"]]);
    expect(parseFilters((key) => params.get(key) ?? null)).toEqual({
      protocol: "anthropic_messages",
    });
  });

  it("ignores blank values so an empty box is not a filter", () => {
    const params = new Map([["provider_id", "   "]]);
    expect(parseFilters((key) => params.get(key) ?? null)).toEqual({});
    expect(activeFilterCount({})).toBe(0);
    expect(activeFilterCount({ provider_id: "a", model: "b" })).toBe(2);
  });
});

describe("range", () => {
  it("falls back to 7d for an unknown preset", () => {
    expect(parseRange("nonsense")).toBe("7d");
    expect(parseRange(null)).toBe("7d");
    expect(parseRange("24h")).toBe("24h");
  });

  it("omits both bounds for 'all' rather than sending from_ms: 0", () => {
    // An explicit zero is a filter the backend must honour; omission lets it
    // answer over its own retention window.
    expect(rangeParams("all", 1_700_000_000_000)).toEqual({});
  });

  it("computes a closed window for the presets", () => {
    expect(rangeParams("24h", 1_700_000_000_000)).toEqual({
      from_ms: 1_700_000_000_000 - 86_400_000,
      to_ms: 1_700_000_000_000,
    });
  });
});

describe("formatting", () => {
  it("renders an unobserved total as an em dash, never as zero", () => {
    expect(formatTokens(null)).toBe("—");
    expect(formatTokens(0)).toBe("0");
    expect(formatTokens(1234567)).toBe("1,234,567");
  });

  it("says so when there is no watermark at all", () => {
    expect(formatWatermark(null)).toBe("尚无观测");
    expect(formatWatermark(1_700_000_000_000)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}Z$/u);
  });

  it("tones the three confidences apart", () => {
    expect(confidenceTone("exact")).toBe("good");
    expect(confidenceTone("partial")).toBe("warn");
    expect(confidenceTone("unknown")).toBe("muted");
  });

  it("guards share against a zero total", () => {
    expect(shareOf(3, 0)).toBe(0);
    expect(shareOf(3, 12)).toBe(0.25);
  });

  it("falls back to a real dimension for an unknown one", () => {
    expect(parseDimension("nope")).toBe("provider_id");
    expect(parseDimension("public_model")).toBe("public_model");
  });
});
