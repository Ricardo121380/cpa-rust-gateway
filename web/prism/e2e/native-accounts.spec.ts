import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function approve(page: Page) {
  const id = await page.locator("[data-native-session-id]").getAttribute("data-native-session-id");
  if (!id) throw new Error("missing session");
  await page.evaluate(async (session) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.approveNativeDeviceForTest(session);
  }, id);
}

test("native SSO import stays channel-owned and guarded Back returns to the same account", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();
  const importSheet = page.getByRole("dialog", { name: "授权或导入账号" });
  await importSheet.getByLabel("渠道", { exact: true }).selectOption("grok.console");
  const material = importSheet.getByLabel("SSO 凭据", { exact: true });
  await material.fill(JSON.stringify({ sso_token: "synthetic-sso", probe_model: "grok-4.6" }));
  await importSheet.locator(".sheet-footer").getByRole("button", { name: "导入账号" }).click();
  await expect(importSheet).toContainText("导入结果");
  await importSheet.getByRole("button", { name: "完成" }).click();

  const row = page.locator('[data-account-key^="grok-fixture-"]').first();
  await expect(row).toContainText("session.member@example.test");
  await expect(row).not.toContainText("synthetic-sso");
  await row.getByRole("button", { name: "详情" }).click();
  const detail = page.getByRole("dialog", { name: "账号详情" });
  await detail.getByRole("navigation", { name: "账号详情分类" }).getByRole("button", { name: "配置" }).click();
  await detail.getByRole("button", { name: "更新凭据" }).click();
  const update = page.getByRole("dialog", { name: "更新 SSO 凭据" });
  const replacement = update.getByLabel("SSO 凭据");
  await replacement.fill("replacement-sso-material");
  await update.getByRole("button", { name: "返回" }).click();
  await page.getByRole("alertdialog", { name: "放弃未保存的修改？" }).getByRole("button", { name: "继续编辑" }).click();
  await expect(replacement).toHaveValue("replacement-sso-material");
  await update.getByRole("button", { name: "返回" }).click();
  await page.getByRole("alertdialog", { name: "放弃未保存的修改？" }).getByRole("button", { name: "放弃修改" }).click();
  await expect(page.getByRole("dialog", { name: "账号详情" })).toBeVisible();
});

test("Grok first authorization waits for consent and reauthorizes the same account", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();
  const chooser = page.getByRole("dialog", { name: "授权或导入账号" });
  await chooser.getByLabel("渠道", { exact: true }).selectOption("grok.build");
  await chooser.getByRole("button", { name: "授权登录", exact: true }).click();
  await page.getByRole("button", { name: "开始 Grok 授权", exact: true }).click();
  await expect(page.getByRole("link", { name: "打开 Grok 授权页面" })).toBeVisible();
  await expect(page.locator("[data-native-session-id]")).toHaveText("等待 Grok 授权");
  await approve(page);
  await expect(page.locator("[data-native-session-id]")).toHaveText("授权已保存");
  await expect(page.getByRole("dialog")).toContainText("authorized.member@example.test");
  await page.getByRole("button", { name: "关闭", exact: true }).click();

  const first = page.locator('[data-account-key^="grok-fixture-"]').first();
  await expect(first).toContainText("authorized.member@example.test");
  const firstId = await first.getAttribute("data-account-key");
  await first.getByRole("button", { name: "重新授权", exact: true }).click();
  await page.getByRole("button", { name: "开始 Grok 授权", exact: true }).click();
  await approve(page);
  await expect(page.locator("[data-native-session-id]")).toHaveText("授权已保存");
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.locator('[data-account-key^="grok-fixture-"]')).toHaveCount(1);
  await expect(page.locator('[data-account-key^="grok-fixture-"]')).toHaveAttribute("data-account-key", firstId!);
});
