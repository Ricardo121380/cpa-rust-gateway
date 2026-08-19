import { describe, expect, it } from "vitest";
import { buildJsonl, EXPORT_FORMAT, exportFilename, toExportRow, type ExportMeta } from "./export";
import type { LedgerRow } from "./model";

function ledger(over: Partial<LedgerRow> = {}): LedgerRow {
  return {
    ledger_id: 12,
    request_id: "req-1",
    response_id: "resp-1",
    provider_id: "prov-a",
    channel_id: "ch-a",
    account_id: "acct-a",
    model: "minimax-m3",
    input_tokens: 100,
    output_tokens: 20,
    reasoning_tokens: null,
    cache_read_tokens: 0,
    cache_creation_tokens: 0,
    cached_tokens: 0,
    occurred_at_ms: 1_700_000_000_000,
    catalog_version_id: "cat-1",
    cost_microunits: 4200,
    cost_confidence: "exact",
    ...over,
  };
}

const meta: ExportMeta = {
  filters: { provider_id: "prov-a" },
  row_count: 1,
  partial: false,
};

describe("toExportRow", () => {
  it("carries both time forms so the file serves machines and humans", () => {
    const row = toExportRow(ledger());
    expect(row.occurred_at_ms).toBe(1_700_000_000_000);
    expect(row.occurred_at_iso).toBe(new Date(1_700_000_000_000).toISOString());
  });

  it("keeps an unobserved token count null instead of coercing it to zero", () => {
    // null is "not observed" and 0 is "observed as zero"; an export that merged
    // them would be unusable for exactly the reconciliation it exists for.
    expect(toExportRow(ledger()).reasoning_tokens).toBeNull();
    expect(toExportRow(ledger()).cache_read_tokens).toBe(0);
  });

  it("exports all six token families, including the ones the table folds away", () => {
    const row = toExportRow(ledger());
    for (const key of [
      "input_tokens",
      "output_tokens",
      "reasoning_tokens",
      "cache_read_tokens",
      "cache_creation_tokens",
      "cached_tokens",
    ] as const) {
      expect(row).toHaveProperty(key);
    }
  });

  it("keeps cost in microunits and does not invent a currency", () => {
    const row = toExportRow(ledger());
    expect(row.cost_microunits).toBe(4200);
    expect(JSON.stringify(row)).not.toMatch(/currency|usd|cny|\$/iu);
  });
});

describe("buildJsonl", () => {
  it("emits a self-describing header followed by one object per row", () => {
    const lines = buildJsonl(meta, [toExportRow(ledger())]).trimEnd().split("\n");
    expect(lines).toHaveLength(2);
    const header = JSON.parse(lines[0] as string) as Record<string, unknown>;
    expect(header["format"]).toBe(EXPORT_FORMAT);
    expect(header["filters"]).toEqual({ provider_id: "prov-a" });
    expect(header["partial"]).toBe(false);
  });

  it("names truncation in the header, so a partial export cannot pass as complete", () => {
    const header = JSON.parse(
      buildJsonl({ ...meta, partial: true }, []).split("\n")[0] as string,
    ) as Record<string, unknown>;
    expect(header["partial"]).toBe(true);
  });

  it("every line parses independently — that is the point of JSONL", () => {
    const text = buildJsonl({ ...meta, row_count: 3 }, [
      toExportRow(ledger({ ledger_id: 1 })),
      toExportRow(ledger({ ledger_id: 2 })),
      toExportRow(ledger({ ledger_id: 3 })),
    ]);
    for (const line of text.trimEnd().split("\n")) {
      expect(() => JSON.parse(line) as unknown).not.toThrow();
    }
  });

  it("leaks no request or response body, because none exists upstream", () => {
    const text = buildJsonl(meta, [toExportRow(ledger())]);
    expect(text).not.toMatch(/"(body|request_body|prompt|messages|content)"/u);
  });
});

describe("exportFilename", () => {
  it("marks a partial file in its own name", () => {
    const now = new Date(1_700_000_000_000);
    expect(exportFilename(meta, now)).toMatch(/^prism-billing-[\d-]+\.jsonl$/u);
    expect(exportFilename({ ...meta, partial: true }, now)).toMatch(/-partial\.jsonl$/u);
  });
});
