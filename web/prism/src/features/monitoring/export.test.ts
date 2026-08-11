import { describe, expect, it } from "vitest";
import type { RequestEventView } from "../../api/proposed-types";
import {
  buildJsonl,
  EXPORT_FORMAT,
  exportFilename,
  toExportRow,
  type ExportMeta,
} from "./export";

const EVENT: RequestEventView = {
  request_id: "req-1",
  occurred_at_ms: 1_800_000_000_000,
  protocol: "openai_responses",
  public_model: "minimax-m3",
  streaming: true,
  outcome: "failed",
  error_code: "upstream_timeout",
  error_scope: "upstream",
  stage: "await_headers",
  retry_decision: "retry_other_credential",
  attempt_count: 2,
  latency_ms: 8100,
  tokens: { input: 900, output: 30, cache_read: 6000 },
  client_key_id: "key-ci",
  credential_id: "cred-relay-key",
  endpoint_id: "ep-relay-a-responses",
};

const META: ExportMeta = {
  from_ms: 1_800_000_000_000,
  to_ms: 1_800_003_600_000,
  status: "failed",
  public_model: "minimax-m3",
  row_count: 1,
  partial: false,
};

describe("export row", () => {
  it("flattens tokens and normalizes absent fields to null", () => {
    const row = toExportRow(EVENT);
    expect(row.tokens_input).toBe(900);
    expect(row.tokens_reasoning).toBeNull();
    expect(row.occurred_at_iso).toBe(new Date(EVENT.occurred_at_ms).toISOString());
  });

  it("carries no field the events contract does not have", () => {
    // The whole value-free premise: if a body ever appeared in the export it
    // would have to have come from somewhere the contract does not provide.
    const keys = Object.keys(toExportRow(EVENT));
    for (const forbidden of ["body", "request_body", "response", "prompt", "messages", "content"]) {
      expect(keys).not.toContain(forbidden);
    }
  });

  it("an event with no tokens at all exports nulls rather than zeros", () => {
    const row = toExportRow({ ...EVENT, tokens: null });
    expect(row.tokens_input).toBeNull();
    expect(row.tokens_cache_read).toBeNull();
  });
});

describe("jsonl file", () => {
  it("first line is a self-describing header, then one row per line", () => {
    const text = buildJsonl(META, [toExportRow(EVENT)]);
    const lines = text.trimEnd().split("\n");
    expect(lines).toHaveLength(2);
    const header = JSON.parse(lines[0]!) as Record<string, unknown>;
    expect(header["format"]).toBe(EXPORT_FORMAT);
    expect(header["window"]).toEqual({ from_ms: META.from_ms, to_ms: META.to_ms });
    expect(header["filters"]).toEqual({ status: "failed", public_model: "minimax-m3" });
    expect(JSON.parse(lines[1]!)).toMatchObject({ request_id: "req-1" });
  });

  it("every line is independently parseable and ends with a newline", () => {
    const text = buildJsonl(META, [toExportRow(EVENT), toExportRow({ ...EVENT, request_id: "req-2" })]);
    expect(text.endsWith("\n")).toBe(true);
    for (const line of text.trimEnd().split("\n")) {
      expect(() => JSON.parse(line)).not.toThrow();
    }
  });

  it("a partial export says so in the header and in the filename", () => {
    const partial = { ...META, partial: true };
    const header = JSON.parse(buildJsonl(partial, []).split("\n")[0]!) as Record<string, unknown>;
    expect(header["partial"]).toBe(true);
    expect(exportFilename(partial, new Date(META.from_ms))).toContain("-partial");
    expect(exportFilename(META, new Date(META.from_ms))).not.toContain("-partial");
  });

  it("an empty export is still a valid file with a header", () => {
    const lines = buildJsonl({ ...META, row_count: 0 }, []).trimEnd().split("\n");
    expect(lines).toHaveLength(1);
    expect(() => JSON.parse(lines[0]!)).not.toThrow();
  });

  it("filename has no characters that need shell or filesystem quoting", () => {
    const name = exportFilename(META, new Date("2026-07-30T19:58:07.000Z"));
    expect(name).toBe("prism-requests-2026-07-30-19-58-07.jsonl");
    expect(name).toMatch(/^[A-Za-z0-9.-]+$/u);
  });
});
