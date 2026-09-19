import { describe, expect, it } from "vitest";
import { accountActionOutcomeLabel, accountActionTargetKey, freezeAccountActionTargets } from "./accountActionModel";

describe("account action target snapshots", () => {
  it("keeps ordinary and native records with the same ID distinct", () => {
    const targets = freezeAccountActionTargets([
      { id: "same", native: false, label: "ordinary" },
      { id: "same", native: true, label: "native" },
      { id: "same", native: false, label: "duplicate" },
    ]);
    expect(targets.map((target) => target.label)).toEqual(["ordinary", "native"]);
    expect(accountActionTargetKey(targets[0]!)).toBe("ordinary:same");
  });

  it("preserves displayed target order when freezing a confirmation", () => {
    expect(freezeAccountActionTargets([
      { id: "b", native: false }, { id: "a", native: false }, { id: "b", native: false },
    ]).map((target) => target.id)).toEqual(["b", "a"]);
  });

  it("keeps saved, applied, unexecuted and uncertain outcomes distinct", () => {
    expect(accountActionOutcomeLabel({ kind: "saved_draft" })).toBe("已保存到草稿");
    expect(accountActionOutcomeLabel({ kind: "saved_unapplied" })).toBe("已保存，待应用");
    expect(accountActionOutcomeLabel({ kind: "unexecuted" })).toBe("未执行");
    expect(accountActionOutcomeLabel({ kind: "unconfirmed", detail: "响应丢失" })).toContain("结果未确认");
  });
});
