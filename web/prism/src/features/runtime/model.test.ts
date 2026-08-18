import { describe, expect, it } from "vitest";
import {
  abnormalRows,
  ageStage,
  availabilityMeta,
  buildAvailabilityMatrix,
  cellKey,
  countByState,
  decisionMeta,
  explainCounts,
  explainScopeHint,
  formatAge,
  formatObservedAt,
  freshnessMeta,
  isProjectionUnavailable,
  isRecoverable,
  normalizeExplainQuery,
  priceEvidenceMeta,
  PROTOCOLS,
  recoverableRows,
  recoveryMeta,
  stateAttr,
  type AvailabilityRow,
} from "./model";

const HOUR = 3_600_000;
const NOW = 1_785_100_000_000;

const ROWS: readonly AvailabilityRow[] = [
  { endpoint_id: "ep-b", credential_id: "cred-2", availability: "quota_blocked" },
  { endpoint_id: "ep-a", credential_id: "cred-2", availability: "cooldown" },
  { endpoint_id: "ep-a", credential_id: "cred-1", availability: "available" },
  { endpoint_id: "ep-b", credential_id: "cred-1", availability: "credential_forbidden" },
];

describe("buildAvailabilityMatrix", () => {
  it("sorts both axes and indexes every returned pair", () => {
    const matrix = buildAvailabilityMatrix(ROWS);
    expect(matrix.endpoints).toEqual(["ep-a", "ep-b"]);
    expect(matrix.credentials).toEqual(["cred-1", "cred-2"]);
    expect(matrix.cells.get(cellKey("ep-b", "cred-1"))?.availability).toBe(
      "credential_forbidden",
    );
  });

  it("leaves pairs the projection did not return absent (sparse, not 'available')", () => {
    const matrix = buildAvailabilityMatrix([
      { endpoint_id: "ep-a", credential_id: "cred-1", availability: "available" },
      { endpoint_id: "ep-b", credential_id: "cred-2", availability: "cooldown" },
    ]);
    expect(matrix.endpoints).toEqual(["ep-a", "ep-b"]);
    expect(matrix.credentials).toEqual(["cred-1", "cred-2"]);
    expect(matrix.cells.get(cellKey("ep-a", "cred-2"))).toBeUndefined();
    expect(matrix.cells.size).toBe(2);
  });

  it("is empty for an empty projection", () => {
    const matrix = buildAvailabilityMatrix([]);
    expect(matrix.endpoints).toEqual([]);
    expect(matrix.credentials).toEqual([]);
  });
});

describe("availabilityMeta", () => {
  it("gives every frozen state a distinct glyph", () => {
    const glyphs = [
      "available",
      "cooldown",
      "circuit_open",
      "quota_blocked",
      "credential_forbidden",
      "recovery_required",
    ].map((state) => availabilityMeta(state).glyph);
    expect(new Set(glyphs).size).toBe(6);
  });

  it("reports an unknown member as itself instead of coercing it", () => {
    expect(availabilityMeta("teleported").tone).toBe("muted");
    expect(stateAttr("teleported")).toBe("unknown");
    expect(stateAttr("cooldown")).toBe("cooldown");
  });
});

describe("countByState / abnormalRows", () => {
  it("counts in enum order and drops zero buckets", () => {
    expect(countByState(ROWS)).toEqual([
      { state: "available", count: 1 },
      { state: "cooldown", count: 1 },
      { state: "quota_blocked", count: 1 },
      { state: "credential_forbidden", count: 1 },
    ]);
    expect(countByState([...ROWS, ROWS[0] as AvailabilityRow])).toContainEqual({
      state: "quota_blocked",
      count: 2,
    });
  });

  it("orders known states by the enum and appends unknown ones", () => {
    const counts = countByState([
      ...ROWS,
      { endpoint_id: "ep-c", credential_id: "cred-9", availability: "teleported" },
    ]);
    expect(counts.map((entry) => entry.state)).toEqual([
      "available",
      "cooldown",
      "quota_blocked",
      "credential_forbidden",
      "teleported",
    ]);
  });

  it("abnormalRows drops available only", () => {
    expect(abnormalRows(ROWS).map((row) => row.availability)).toEqual([
      "quota_blocked",
      "cooldown",
      "credential_forbidden",
    ]);
  });
});

describe("recovery entry points", () => {
  it("offers recovery for quota_blocked and credential_forbidden only", () => {
    expect(isRecoverable("quota_blocked")).toBe(true);
    expect(isRecoverable("credential_forbidden")).toBe(true);
    expect(isRecoverable("circuit_open")).toBe(false);
    expect(isRecoverable("recovery_required")).toBe(false);
    expect(isRecoverable("available")).toBe(false);
  });

  it("sorts recoverable pairs by endpoint then credential", () => {
    expect(recoverableRows(ROWS).map((row) => `${row.endpoint_id}/${row.credential_id}`)).toEqual([
      "ep-b/cred-1",
      "ep-b/cred-2",
    ]);
  });

  it("keeps the three outcomes distinct and never calls a probe a recovery", () => {
    expect(recoveryMeta("probe_scheduled").tone).toBe("good");
    expect(recoveryMeta("recovery_required").tone).toBe("tint");
    expect(recoveryMeta("rejected").tone).toBe("serious");
    expect(recoveryMeta("probe_scheduled").label).not.toBe(
      recoveryMeta("recovery_required").label,
    );
    expect(recoveryMeta("whatever").tone).toBe("muted");
  });
});

describe("catalog lifecycle", () => {
  it("buckets ages against 6h / 24h / 72h", () => {
    expect(ageStage(0, NOW)).toBe("unobserved");
    expect(ageStage(NOW - 5 * HOUR, NOW)).toBe("fresh");
    expect(ageStage(NOW - 6 * HOUR, NOW)).toBe("aging");
    expect(ageStage(NOW - 23 * HOUR, NOW)).toBe("aging");
    expect(ageStage(NOW - 24 * HOUR, NOW)).toBe("refresh_due");
    expect(ageStage(NOW - 71 * HOUR, NOW)).toBe("refresh_due");
    expect(ageStage(NOW - 72 * HOUR, NOW)).toBe("hard_expired");
  });

  it("clamps clock skew instead of reporting a negative age", () => {
    expect(ageStage(NOW + 10 * HOUR, NOW)).toBe("fresh");
    expect(formatAge(NOW + 10 * HOUR, NOW)).toBe("刚刚");
  });

  it("formats relative age coarsely", () => {
    expect(formatAge(0, NOW)).toBe("从未观测");
    expect(formatAge(NOW - 30_000, NOW)).toBe("刚刚");
    expect(formatAge(NOW - 15 * 60_000, NOW)).toBe("15 分钟前");
    expect(formatAge(NOW - 5 * HOUR, NOW)).toBe("5 小时前");
    expect(formatAge(NOW - 47 * HOUR, NOW)).toBe("47 小时前");
    expect(formatAge(NOW - 72 * HOUR, NOW)).toBe("3 天前");
  });

  it("formats the absolute observation in UTC", () => {
    expect(formatObservedAt(1_785_100_000_000)).toBe("2026-07-26 21:06:40Z");
    expect(formatObservedAt(0)).toBe("—");
  });

  it("maps the four freshness members and flags anything else", () => {
    expect(freshnessMeta("fresh").tone).toBe("good");
    expect(freshnessMeta("stale").tone).toBe("warn");
    expect(freshnessMeta("expired").tone).toBe("serious");
    expect(freshnessMeta("missing").tone).toBe("muted");
    expect(freshnessMeta("moldy").label).toBe("未知");
  });
});

describe("route explain", () => {
  it("rejects blank ids and trims the rest", () => {
    expect(
      normalizeExplainQuery({ route_id: "  ", requested_model: "m", protocol: "openai_responses" }),
    ).toBeUndefined();
    expect(
      normalizeExplainQuery({ route_id: "r", requested_model: " ", protocol: "openai_responses" }),
    ).toBeUndefined();
    expect(
      normalizeExplainQuery({
        route_id: " route-1 ",
        requested_model: " glm-5-air ",
        protocol: "anthropic_messages",
      }),
    ).toEqual({
      route_id: "route-1",
      requested_model: "glm-5-air",
      protocol: "anthropic_messages",
    });
  });

  it("counts decisions and keeps unknown ones visible", () => {
    expect(
      explainCounts([
        { candidate_id: "a", decision: "selected", price_evidence: "dominant" },
        {
          candidate_id: "b",
          decision: "excluded",
          reason: "NoEligibleCredential",
          price_evidence: "not_evaluated",
        },
        { candidate_id: "c", decision: "deferred", price_evidence: "disabled" },
      ]),
    ).toEqual({ selected: 1, excluded: 1, other: 1 });
  });

  it("tones the two decisions apart", () => {
    expect(decisionMeta("selected").tone).toBe("good");
    expect(decisionMeta("excluded").tone).toBe("muted");
    expect(decisionMeta("deferred").tone).toBe("muted");
    expect(decisionMeta("deferred").label).toBe("未知");
  });

  it("offers all three contract protocols, including chat completions", () => {
    // Missing openai_chat_completions here meant Explain could not be run for
    // that path at all; the drift gate cannot see a literal a page omits.
    expect([...PROTOCOLS]).toEqual([
      "openai_chat_completions",
      "openai_responses",
      "anthropic_messages",
    ]);
  });

  it("omits provider_id entirely when blank rather than sending an empty one", () => {
    const normalized = normalizeExplainQuery({
      route_id: "route-1",
      requested_model: "m",
      protocol: "openai_responses",
      provider_id: "   ",
    });
    expect(normalized).toBeDefined();
    expect(Object.hasOwn(normalized ?? {}, "provider_id")).toBe(false);
  });

  it("keeps and trims a provided provider_id", () => {
    expect(
      normalizeExplainQuery({
        route_id: "route-1",
        requested_model: "m",
        protocol: "openai_responses",
        provider_id: " prov-a ",
      })?.provider_id,
    ).toBe("prov-a");
  });
});

describe("priceEvidenceMeta", () => {
  it("covers all seven rate_dominance_v1 values with distinct glyphs", () => {
    const values = [
      "dominant",
      "equal",
      "dominated",
      "incomparable",
      "unpriced",
      "not_evaluated",
      "disabled",
    ];
    const glyphs = values.map((value) => priceEvidenceMeta(value).glyph);
    expect(new Set(glyphs).size).toBe(values.length);
    for (const value of values) {
      expect(priceEvidenceMeta(value).label).not.toBe("未知");
    }
  });

  it("does not colour the absences as verdicts", () => {
    // not_evaluated / disabled mean no comparison happened. A status tone would
    // read as a result the backend never produced.
    expect(priceEvidenceMeta("not_evaluated").tone).toBe("muted");
    expect(priceEvidenceMeta("disabled").tone).toBe("muted");
  });

  it("separates cheapest from dearest from unpriced", () => {
    expect(priceEvidenceMeta("dominant").tone).toBe("good");
    expect(priceEvidenceMeta("dominated").tone).toBe("warn");
    // unpriced is a gap in the catalog, never "free".
    expect(priceEvidenceMeta("unpriced").tone).toBe("warn");
  });

  it("renders an unknown value honestly instead of guessing a neighbour", () => {
    expect(priceEvidenceMeta("some_future_value").label).toBe("未知");
  });
});

describe("explainScopeHint", () => {
  it("names the two Provider-scope failures the contract can return", () => {
    expect(explainScopeHint("provider_scope_required")).toBeTypeOf("string");
    expect(explainScopeHint("provider_mismatch")).toBeTypeOf("string");
  });

  it("stays out of the way for every other error", () => {
    expect(explainScopeHint("management_internal_error")).toBeUndefined();
    expect(explainScopeHint(undefined)).toBeUndefined();
  });
});

describe("isProjectionUnavailable", () => {
  it("recognises the fail-closed facade response", () => {
    expect(isProjectionUnavailable({ kind: "unavailable", status: 503 })).toBe(true);
    expect(isProjectionUnavailable({ kind: "unknown", status: 503 })).toBe(true);
    expect(isProjectionUnavailable({ kind: "network", status: undefined })).toBe(false);
    expect(isProjectionUnavailable({ kind: "conflict", status: 409 })).toBe(false);
    expect(isProjectionUnavailable(new Error("boom"))).toBe(false);
    expect(isProjectionUnavailable(undefined)).toBe(false);
  });
});
