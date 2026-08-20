import { describe, expect, it } from "vitest";
import {
  formatCatalogEntries,
  formatRate,
  isEffective,
  isPolicyUnset,
  MAX_ENTRIES,
  parseCatalogEntries,
  sortCatalogs,
  sourceLabel,
  type Catalog,
  type CatalogEntry,
} from "./model";

function entry(over: Partial<CatalogEntry> = {}): CatalogEntry {
  return {
    provider_id: "prov-a",
    channel_id: "ch-a",
    model: "minimax-m3",
    input_microunits_per_million: 1_500_000,
    output_microunits_per_million: 6_000_000,
    reasoning_microunits_per_million: 0,
    cache_read_microunits_per_million: 0,
    cache_creation_microunits_per_million: 0,
    cached_microunits_per_million: 0,
    ...over,
  };
}

function catalog(over: Partial<Catalog> = {}): Catalog {
  return {
    catalog_version_id: "cat-1",
    effective_at_ms: 1_700_000_000_000,
    created_at_ms: 1_699_000_000_000,
    source: "operator",
    entries: [entry()],
    ...over,
  };
}

describe("parseCatalogEntries", () => {
  it("accepts a well-formed array and returns typed entries", () => {
    const parsed = parseCatalogEntries(JSON.stringify([entry()]));
    expect(parsed.ok).toBe(true);
    if (parsed.ok) {
      expect(parsed.entries).toHaveLength(1);
      expect(parsed.entries[0]?.input_microunits_per_million).toBe(1_500_000);
    }
  });

  it("rejects an empty catalog — the contract's minItems is 1, not 0", () => {
    expect(parseCatalogEntries("[]").ok).toBe(false);
    expect(parseCatalogEntries("   ").ok).toBe(false);
  });

  it("rejects more entries than the contract allows", () => {
    const many = JSON.stringify(
      Array.from({ length: MAX_ENTRIES + 1 }, (_, index) => entry({ model: `m-${index}` })),
    );
    const parsed = parseCatalogEntries(many);
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain(String(MAX_ENTRIES));
    }
  });

  it("names the failing entry by position, not just 'invalid'", () => {
    // A 400 on a 512-row paste is useless; the operator needs the row number.
    const parsed = parseCatalogEntries(
      JSON.stringify([entry(), entry({ model: "" }), entry({ model: "c" })]),
    );
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain("第 2 条");
      expect(parsed.reason).toContain("model");
    }
  });

  it("rejects a non-integer or negative rate rather than rounding it", () => {
    expect(
      parseCatalogEntries(JSON.stringify([entry({ input_microunits_per_million: 1.5 })])).ok,
    ).toBe(false);
    expect(
      parseCatalogEntries(JSON.stringify([entry({ output_microunits_per_million: -1 })])).ok,
    ).toBe(false);
  });

  it("accepts a zero rate — free is a real price, absent is not", () => {
    expect(parseCatalogEntries(JSON.stringify([entry({ input_microunits_per_million: 0 })])).ok).toBe(
      true,
    );
  });

  it("requires every one of the six rate fields", () => {
    const partial = { ...entry() } as Record<string, unknown>;
    delete partial["cached_microunits_per_million"];
    const parsed = parseCatalogEntries(JSON.stringify([partial]));
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain("cached_microunits_per_million");
    }
  });

  it("catches a duplicate provider/channel/model before the round trip", () => {
    const parsed = parseCatalogEntries(JSON.stringify([entry(), entry()]));
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain("重复");
    }
  });

  it("rejects a non-array top level with a usable message", () => {
    const parsed = parseCatalogEntries(JSON.stringify({ entries: [entry()] }));
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain("数组");
    }
  });

  it("reports a JSON syntax error instead of a generic failure", () => {
    const parsed = parseCatalogEntries("[{");
    expect(parsed.ok).toBe(false);
    if (!parsed.ok) {
      expect(parsed.reason).toContain("JSON");
    }
  });

  it("round-trips through the display format", () => {
    const parsed = parseCatalogEntries(formatCatalogEntries([entry()]));
    expect(parsed.ok).toBe(true);
  });
});

describe("isEffective", () => {
  it("treats a catalog dated now or earlier as bindable", () => {
    expect(isEffective(catalog({ effective_at_ms: 1000 }), 1000)).toBe(true);
    expect(isEffective(catalog({ effective_at_ms: 999 }), 1000)).toBe(true);
  });

  it("treats a future catalog as not bindable", () => {
    // set_routing_price_policy fails with RoutingPriceCatalogNotEffective, so
    // the picker must not offer it.
    expect(isEffective(catalog({ effective_at_ms: 1001 }), 1000)).toBe(false);
  });
});

describe("sortCatalogs", () => {
  it("orders newest-effective first with a stable tiebreak", () => {
    const sorted = sortCatalogs([
      catalog({ catalog_version_id: "b", effective_at_ms: 100, created_at_ms: 1 }),
      catalog({ catalog_version_id: "a", effective_at_ms: 300, created_at_ms: 1 }),
      catalog({ catalog_version_id: "c", effective_at_ms: 100, created_at_ms: 1 }),
    ]);
    expect(sorted.map((row) => row.catalog_version_id)).toEqual(["a", "b", "c"]);
  });

  it("does not mutate its input", () => {
    const input = [catalog({ catalog_version_id: "b", effective_at_ms: 1 }), catalog()];
    sortCatalogs(input);
    expect(input[0]?.catalog_version_id).toBe("b");
  });
});

describe("isPolicyUnset", () => {
  it("recognises the not-configured 404 as a state, not an error", () => {
    expect(isPolicyUnset({ status: 404, code: "management_resource_not_found" })).toBe(true);
  });

  it("does NOT swallow the access-denied 404, which means a dead session", () => {
    // 404 management_access_denied is the gateway's answer to a disallowed
    // browser origin. Treating it as "no policy" would paint a calm empty
    // state over a session the client is about to reset.
    expect(isPolicyUnset({ status: 404, code: "management_access_denied" })).toBe(false);
  });

  it("ignores everything else", () => {
    expect(isPolicyUnset({ status: 500, code: "management_internal_error" })).toBe(false);
    expect(isPolicyUnset(undefined)).toBe(false);
    expect(isPolicyUnset(new Error("boom"))).toBe(false);
  });
});

describe("formatting", () => {
  it("groups rate digits and applies no currency", () => {
    expect(formatRate(1_500_000)).toBe("1,500,000");
    expect(formatRate(1_500_000)).not.toMatch(/[$¥€]/u);
  });

  it("labels the writable sources and passes an unknown one through", () => {
    expect(sourceLabel("operator")).toBe("运维录入");
    expect(sourceLabel("test")).toBe("测试");
    expect(sourceLabel("future_source")).toBe("future_source");
  });
});
