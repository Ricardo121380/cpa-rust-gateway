import { expect, it } from "vitest";
import { isInternalLabel, referenceText, resourceOption, resourceName } from "./resourceNames";

it("uses readable production labels for legacy phase IDs and opaque accounts", () => {
  expect(resourceName("p12-06-codex-bridge-credential", "account")).toBe("Codex 账号");
  expect(resourceName("p12-12-production-grok-build-upstream", "upstream")).toBe("Grok Build 提供商");
  expect(resourceName("grok-" + "b".repeat(32), "account")).toBe("Grok 账号");
  expect(resourceName("p12-chatgpt-go-test-1786163922", "config")).toBe("ChatGPT Go 配置");
  expect(resourceName("p12-06-codex-upstream", "upstream", "P12-06 official ChatGPT Codex")).toBe("官方 ChatGPT Codex 提供商");
  expect(resourceName("p12-06-codex-group", "group", "P12-06 official Codex bridge staging")).toBe("官方 Codex 访问组");
  expect(resourceName("p12-06-grok-4-route","route")).toBe("Grok 4 路由");
  expect(resourceName("p12-account","account","p12-member@example.test")).toBe("p12-member@example.test");
});
it("preserves custom labels and exact protocol model names", () => {
  expect(resourceName("p12-12-production-route", "route", "家庭工作区")).toBe("家庭工作区");
  for (const id of ["gpt-5.5", "grok-4.6", "p12 research model", "cred-relay-key"]) {
    expect(isInternalLabel(id)).toBe(false);
    expect(resourceName(id)).toBe(id);
  }
});
it("omits invented hashes while retaining exact values for callers", () => {
  const a = "p12-06-codex-bridge-credential";
  const b = "p12-09-codex-bridge-credential";
  expect(resourceName(a, "account")).toBe(resourceName(b, "account"));
  expect(resourceOption(a,"account")).toBe("Codex 账号");
  expect(a).toBe("p12-06-codex-bridge-credential");
  expect(referenceText(`删除 ${a}，保留 gpt-5.5。`)).toBe("删除 Codex 账号，保留 gpt-5.5。");
  expect(resourceName("acceptance-grok-console-1786163922","upstream")).toBe("Grok Console 提供商");
});
