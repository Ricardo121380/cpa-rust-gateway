// Provider 账号池 · 实时 (P13-06B/C) on the runtime page.
//
// The assertions target the scope split (read needs no version, act does), the
// two independent status axes, and the two action outcomes that are answers
// rather than failures.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("the pool reads with no config version, but its actions do not", async ({ page }) => {
  await unlock(page);
  await navigate(page, "运行时");

  // listProviderAccountPools declares no X-Config-Version, so the table is
  // there before anything is selected — a blanket "pick a version" state
  // would be false for it.
  await expect(page.getByText("Provider 账号池 · 实时")).toBeVisible();
  await expect(page.locator("tr", { hasText: "cred-relay-key" })).toBeVisible();
  await expect(page.locator(".rt-card").first()).toContainText("本表不需要配置版本");
  await expect(page.locator(".rt-card").first()).toContainText("操作按钮不可用");

  const cool = page.locator("tr", { hasText: "cred-relay-key" }).getByRole("button", { name: "冷却" });
  await expect(cool).toBeDisabled();

  await selectDraft(page);
  await expect(cool).toBeEnabled();
});

test("auth status and runtime status stay two axes, never one health value", async ({ page }) => {
  await unlock(page);
  await navigate(page, "运行时");

  // cred-grok-oauth is reauth_required on the auth axis and unauthorized on
  // the runtime one. Both must be visible; neither may be merged away.
  const row = page.locator("tr", { hasText: "cred-grok-oauth" });
  await expect(row.locator('.rt-chip[data-state="reauth_required"]')).toBeVisible();
  await expect(row.locator('.rt-chip[data-state="unauthorized"]')).toBeVisible();
  await expect(page.locator(".rt-card").first()).toContainText("两个独立维度");
});

test("cooling names the exact account and enforces the contract's window", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  await page.locator("tr", { hasText: "cred-relay-key" }).getByRole("button", { name: "冷却" }).click();
  const sheet = page.getByRole("dialog");
  // An action on one account out of a pool must say which one.
  await expect(sheet).toContainText("精确到账号");
  await expect(sheet).toContainText("cred-relay-key");

  // Below the contract's floor. The input carries min/max, so the browser's own
  // constraint validation refuses the submit before any handler runs — the
  // sheet stays open and nothing is sent. validCooldown is the second line, for
  // values that never pass through this input at all (unit-tested separately).
  const field = sheet.getByLabel("冷却时长", { exact: false });
  await field.fill("500");
  await sheet.getByRole("button", { name: "确认冷却" }).click();
  await expect(field).toHaveJSProperty("validity.rangeUnderflow", true);
  await expect(page.locator(".action-notice")).toHaveCount(0);
  await expect(sheet).toBeVisible();

  await field.fill("60000");
  await sheet.getByRole("button", { name: "确认冷却" }).click();
  await expect(page.locator(".action-notice")).toContainText("已进入冷却");
});

test("a refused recovery is reported as an answer, not an error", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  await page
    .locator("tr", { hasText: "cred-grok-oauth" })
    .getByRole("button", { name: "请求恢复" })
    .click();
  const sheet = page.getByRole("dialog");
  // The sheet promises nothing it cannot deliver.
  await expect(sheet).toContainText("不保证恢复");
  await sheet.getByRole("button", { name: "确认请求恢复" }).click();

  await expect(page.locator(".action-notice")).toContainText("需要人工恢复");
  await expect(page.locator(".action-error")).toHaveCount(0);
});

test("a stale target re-reads the snapshot instead of retrying blind", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  await page.locator("tr", { hasText: "cred-grok-old" }).getByRole("button", { name: "冷却" }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("冷却时长", { exact: false }).fill("60000");
  await sheet.getByRole("button", { name: "确认冷却" }).click();

  await expect(page.locator(".action-error")).toContainText("快照已变");
  await expect(page.locator(".action-error")).toContainText("重新读取");
});
