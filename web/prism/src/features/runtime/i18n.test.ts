import { describe, expect, it } from "vitest";
import {
  authStatusMeta,
  availabilityMeta,
  AUTH_STATUSES,
  AVAILABILITY_STATES,
  CLEARANCE_STATES,
  domainStateMeta,
  EGRESS_STATES,
  freshnessMeta,
  FRESHNESS_STATES,
  localizeMeta,
  pinOutcomeMeta,
  priceEvidenceMeta,
  receiptMeta,
  RECOVERY_STATES,
  runtimeStatusMeta,
  RUNTIME_STATUSES,
  SESSION_STATES,
} from "./model";

/** Every closed vocabulary the UI renders as a chip, paired with its lookup. */
const VOCABULARIES: ReadonlyArray<
  Readonly<{ name: string; values: readonly string[]; meta: (value: string) => ReturnType<typeof availabilityMeta> }>
> = [
  { name: "availability", values: AVAILABILITY_STATES, meta: availabilityMeta },
  { name: "freshness", values: FRESHNESS_STATES, meta: freshnessMeta },
  { name: "auth", values: AUTH_STATUSES, meta: authStatusMeta },
  { name: "runtime", values: RUNTIME_STATUSES, meta: runtimeStatusMeta },
  { name: "recovery", values: RECOVERY_STATES, meta: receiptMeta },
  {
    name: "price evidence",
    values: ["dominant", "equal", "dominated", "incomparable", "unpriced", "not_evaluated", "disabled"],
    meta: priceEvidenceMeta,
  },
  { name: "pin outcome", values: ["succeeded", "rejected", "failed"], meta: pinOutcomeMeta },
  { name: "egress", values: EGRESS_STATES, meta: (v) => domainStateMeta("egress", v) },
  { name: "session", values: SESSION_STATES, meta: (v) => domainStateMeta("session", v) },
  { name: "clearance", values: CLEARANCE_STATES, meta: (v) => domainStateMeta("clearance", v) },
];

describe("state vocabulary translations", () => {
  it("covers every member of every closed vocabulary", () => {
    // This is the guarantee the i18n pack gives for page chrome, reproduced for
    // the enum vocabularies that live beside their enums instead. Adding a state
    // without English fails here rather than shipping a Chinese chip into an
    // English UI.
    const missing: string[] = [];
    for (const vocabulary of VOCABULARIES) {
      for (const value of vocabulary.values) {
        if (vocabulary.meta(value).en === undefined) {
          missing.push(`${vocabulary.name}.${value}`);
        }
      }
    }
    expect(missing).toEqual([]);
  });

  it("keeps the vocabularies apart, which a flat key space could not", () => {
    // `disabled` is three different things and they must translate differently.
    // A single i18n table keyed by the raw enum value would collapse them.
    expect(authStatusMeta("disabled").en?.detail).not.toBe(
      priceEvidenceMeta("disabled").en?.detail,
    );
    expect(domainStateMeta("egress", "disabled").en?.detail).not.toBe(
      priceEvidenceMeta("disabled").en?.detail,
    );
    // …and the same holds for the states that read as "fine" in every language.
    expect(domainStateMeta("session", "active").en?.label).toBe("Active");
    expect(authStatusMeta("active").en?.label).toBe("Usable");
  });

  it("swaps only the text, never the glyph or the tone", () => {
    const zh = availabilityMeta("circuit_open");
    const en = localizeMeta(zh, "en");
    expect(en.label).toBe("Circuit open");
    expect(en.glyph).toBe(zh.glyph);
    expect(en.tone).toBe(zh.tone);
  });

  it("falls back to the Chinese rather than to an empty string", () => {
    // A visibly untranslated label is honest; a blank one reads as a rendering
    // fault and hides the fact that a translation is missing.
    const untranslated = { label: "标签", glyph: "·", tone: "muted" as const, detail: "细节" };
    expect(localizeMeta(untranslated, "en").label).toBe("标签");
    expect(localizeMeta(availabilityMeta("available"), "zh").label).toBe("可用");
  });
});
