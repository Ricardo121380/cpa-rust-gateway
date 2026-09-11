import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock, clearVersionForTest } from "./helpers";

async function inspect(page: Page, account: string) {
  await page.getByRole("textbox", { name: "搜索已加载账号" }).fill(account);
  await page.locator(".account-desktop").getByRole("button", { name: "详情", exact: true }).click();
}

test("accounts read without a version, while cooldown requires a selected version and confirmation", async ({ page }) => {
  await unlock(page);
  await clearVersionForTest(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-relay-key");
  await expect(page.getByRole("dialog").getByRole("button", { name: "冷却账号" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await selectDraft(page);
  await inspect(page, "cred-relay-key");
  await page.getByRole("dialog").getByRole("button", { name: "冷却账号" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("relay-a / ep-relay-a-responses / cred-relay-key");
  await confirm.getByLabel("冷却时长", { exact: false }).fill("500");
  await confirm.getByRole("button", { name: "确认冷却" }).click();
  await expect(confirm).toBeVisible();
  await confirm.getByLabel("冷却时长", { exact: false }).fill("60000");
  await confirm.getByRole("button", { name: "确认冷却" }).click();
  await expect(page.getByRole("status")).toContainText("冷却");
});

test("recovery and target conflicts remain runtime facts", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-grok-oauth");
  await page.getByRole("dialog").getByRole("button", { name: "请求恢复", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认请求恢复" }).click();
  await expect(page.getByRole("status")).toContainText("需要人工恢复");
  await inspect(page, "cred-grok-old");
  await page.getByRole("dialog").getByRole("button", { name: "冷却账号" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认冷却" }).click();
  await expect(page.getByRole("alert")).toContainText("目标快照已改变");
  await expect(page.locator(".conflict-bar")).toHaveCount(0);
});
