// JSONL export of the billing ledger. Client-side on purpose: the rows are
// already in memory from the ledger query, so a server-side export would be a
// second source of truth for the same window — and no such endpoint exists.
// What the user gets is exactly what the table showed.
//
// The export carries the fields the table renders and nothing more: closed
// enums, identifiers, counters, timestamps. No request or response bodies —
// there are none in the contract to leak, and the file must not imply
// otherwise.
import type { LedgerRow } from "./model";

/** The exact field set the ledger table displays, plus the four token families
 *  the table folds away. Adding a field here is a deliberate act. */
export type ExportRow = Readonly<{
  ledger_id: number;
  request_id: string;
  response_id: string;
  occurred_at_ms: number;
  occurred_at_iso: string;
  provider_id: string;
  channel_id: string;
  account_id: string;
  model: string;
  input_tokens: number | null;
  output_tokens: number | null;
  reasoning_tokens: number | null;
  cache_read_tokens: number | null;
  cache_creation_tokens: number | null;
  cached_tokens: number | null;
  catalog_version_id: string | null;
  /** Microunits. The contract names no currency anywhere, so the file does not
   *  either — a consumer that wants money must supply the unit itself. */
  cost_microunits: number | null;
  cost_confidence: string;
}>;

export function toExportRow(row: LedgerRow): ExportRow {
  return {
    ledger_id: row.ledger_id,
    request_id: row.request_id,
    response_id: row.response_id,
    occurred_at_ms: row.occurred_at_ms,
    // Both forms: the epoch for machines, ISO for a human opening the file.
    occurred_at_iso: new Date(row.occurred_at_ms).toISOString(),
    provider_id: row.provider_id,
    channel_id: row.channel_id,
    account_id: row.account_id,
    model: row.model,
    input_tokens: row.input_tokens,
    output_tokens: row.output_tokens,
    reasoning_tokens: row.reasoning_tokens,
    cache_read_tokens: row.cache_read_tokens,
    cache_creation_tokens: row.cache_creation_tokens,
    cached_tokens: row.cached_tokens,
    catalog_version_id: row.catalog_version_id,
    cost_microunits: row.cost_microunits,
    cost_confidence: row.cost_confidence,
  };
}

/** A header line makes the file self-describing: which window it covers, which
 *  filters produced it, and how many rows the UI actually held. Without it an
 *  export of a filtered view is indistinguishable from a complete one. */
export type ExportMeta = Readonly<{
  filters: Readonly<Record<string, string>>;
  row_count: number;
  /** true when the ledger had more pages the user never loaded */
  partial: boolean;
}>;

export const EXPORT_FORMAT = "prism.billing-ledger.v1";

export function buildJsonl(meta: ExportMeta, rows: readonly ExportRow[]): string {
  const header = {
    format: EXPORT_FORMAT,
    exported_at_iso: new Date().toISOString(),
    filters: meta.filters,
    row_count: meta.row_count,
    // Named, not silent: an export of the first two pages is not the window.
    partial: meta.partial,
    note: "Value-free by contract: closed enums, identifiers, counters and timestamps only. Costs are microunits; the management contract names no currency. No request or response bodies exist in the source ledger.",
  };
  return [JSON.stringify(header), ...rows.map((row) => JSON.stringify(row))].join("\n") + "\n";
}

export function exportFilename(meta: ExportMeta, now: Date = new Date()): string {
  const stamp = now.toISOString().slice(0, 19).replace(/[:T]/gu, "-");
  return `prism-billing-${stamp}${meta.partial ? "-partial" : ""}.jsonl`;
}

/** Blob + object URL rather than a data: URL — `connect-src 'self'` does not
 *  apply (no fetch involved) and a large data: URL would be a needless string
 *  copy of the whole file. The URL is revoked on the next tick; holding it would
 *  pin the blob in memory for the session. */
export function downloadText(filename: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: "application/x-ndjson" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
