// JSONL export (docs/07 §7.3). Client-side on purpose: the rows are already in
// memory from the events query, so a streaming server export would be a second
// source of truth for the same window — and the streaming endpoint is G3
// ancillary, not delivered. What the user gets is exactly what the table showed.
//
// Redaction is not optional (docs/07 §189: "脱敏默认开 … 导出与展示同一脱敏规则").
// The export therefore carries the same fields the table renders and nothing
// more: closed enums, identifiers, counters, timestamps. No request bodies —
// there are none in the contract to leak, and the file must not imply otherwise.
import type { RequestEventView } from "../../api/proposed-types";

/** The exact field set the monitoring table displays, in display order. Adding a
 *  field here is a deliberate act: it must already be visible in the UI. */
export type ExportRow = Readonly<{
  request_id: string;
  occurred_at_ms: number;
  occurred_at_iso: string;
  public_model: string;
  protocol: string;
  streaming: boolean;
  outcome: string;
  error_code: string | null;
  error_scope: string | null;
  stage: string | null;
  retry_decision: string | null;
  attempt_count: number;
  latency_ms: number | null;
  tokens_input: number | null;
  tokens_output: number | null;
  tokens_reasoning: number | null;
  tokens_cache_read: number | null;
  client_key_id: string;
  credential_id: string | null;
  endpoint_id: string | null;
}>;

export function toExportRow(event: RequestEventView): ExportRow {
  const tokens = event.tokens ?? {};
  return {
    request_id: event.request_id,
    occurred_at_ms: event.occurred_at_ms,
    // Both forms: the epoch for machines, ISO for a human opening the file.
    occurred_at_iso: new Date(event.occurred_at_ms).toISOString(),
    public_model: event.public_model,
    protocol: event.protocol,
    streaming: event.streaming,
    outcome: event.outcome,
    error_code: event.error_code ?? null,
    error_scope: event.error_scope ?? null,
    stage: event.stage ?? null,
    retry_decision: event.retry_decision ?? null,
    attempt_count: event.attempt_count,
    latency_ms: event.latency_ms ?? null,
    tokens_input: tokens.input ?? null,
    tokens_output: tokens.output ?? null,
    tokens_reasoning: tokens.reasoning ?? null,
    tokens_cache_read: tokens.cache_read ?? null,
    client_key_id: event.client_key_id,
    credential_id: event.credential_id ?? null,
    endpoint_id: event.endpoint_id ?? null,
  };
}

/** A header line makes the file self-describing: which window it covers, which
 *  filters produced it, and how many rows the UI actually held. Without it an
 *  export of a filtered view is indistinguishable from a complete one. */
export type ExportMeta = Readonly<{
  from_ms: number;
  to_ms: number;
  status: string;
  public_model: string | null;
  row_count: number;
  /** true when the table had more pages the user never loaded */
  partial: boolean;
}>;

export const EXPORT_FORMAT = "prism.requests.v1";

export function buildJsonl(meta: ExportMeta, rows: readonly ExportRow[]): string {
  const header = {
    format: EXPORT_FORMAT,
    exported_at_iso: new Date().toISOString(),
    window: { from_ms: meta.from_ms, to_ms: meta.to_ms },
    filters: { status: meta.status, public_model: meta.public_model },
    row_count: meta.row_count,
    // Named, not silent: an export of the first two pages is not the window.
    partial: meta.partial,
    note: "Value-free by contract: closed enums, identifiers and counters only. No request or response bodies exist in the source events.",
  };
  return [JSON.stringify(header), ...rows.map((row) => JSON.stringify(row))].join("\n") + "\n";
}

export function exportFilename(meta: ExportMeta, now: Date = new Date()): string {
  const stamp = now.toISOString().slice(0, 19).replace(/[:T]/gu, "-");
  return `prism-requests-${stamp}${meta.partial ? "-partial" : ""}.jsonl`;
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
