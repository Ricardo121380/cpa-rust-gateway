import { expect, it } from "vitest";
import { isInternalLabel, resourceCode, resourceName } from "./resourceNames";

it("uses readable production labels for legacy phase IDs and opaque accounts", () => {
  expect(resourceName("p12-06-codex-bridge-credential", "account")).toBe("Codex 桥接 账号");
  expect(resourceName("p12-12-production-grok-build-upstream", "upstream")).toBe("Grok Build 上游");
  expect(resourceName("grok-" + "b".repeat(32), "account")).toBe("Grok 账号");
});
it("preserves custom labels and exact protocol model names", () => {
  expect(resourceName("p12-12-production-route", "route", "家庭工作区")).toBe("家庭工作区");
  for (const id of ["gpt-5.5", "grok-4.6", "p12 research model", "cred-relay-key"]) {
    expect(isInternalLabel(id)).toBe(false);
    expect(resourceName(id)).toBe(id);
  }
});
it("distinguishes legacy generations with stable display codes without rewriting either ID", () => {
  const a = "p12-06-codex-bridge-credential";
  const b = "p12-09-codex-bridge-credential";
  expect(resourceName(a, "account")).toBe(resourceName(b, "account"));
  expect(resourceCode(a)).not.toBe(resourceCode(b));
  expect(resourceCode(a)).toMatch(/^[A-F0-9]{8}$/u);
});
