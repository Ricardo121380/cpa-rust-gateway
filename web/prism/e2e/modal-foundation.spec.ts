import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("discard confirmation isolates a dirty account-import form and restores its field", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();

  const sheet = page.getByRole("dialog", { name: "授权或导入账号" });
  const material = sheet.locator("textarea[name=secret]");
  await material.fill("synthetic-transient-material");
  await sheet.getByRole("button", { name: "取消", exact: true }).click();

  const discard = page.getByRole("alertdialog", { name: "放弃未保存的修改？" });
  await expect(discard).toBeVisible();
  await expect(sheet.locator(".sheet-heading")).toHaveAttribute("aria-hidden", "true");
  await expect(sheet.locator(".sheet-body")).toHaveAttribute("inert", "");

  await discard.getByRole("button", { name: "继续编辑" }).focus();
  await page.keyboard.press("Shift+Tab");
  await expect(discard.getByRole("button", { name: "放弃修改" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(discard.getByRole("button", { name: "继续编辑" })).toBeFocused();

  await discard.getByRole("button", { name: "继续编辑" }).click();
  await expect(material).toHaveValue("synthetic-transient-material");
  await expect(material).toBeFocused();

  await sheet.getByRole("button", { name: "取消", exact: true }).click();
  await page.getByRole("alertdialog", { name: "放弃未保存的修改？" }).getByRole("button", { name: "放弃修改" }).click();
  await expect(sheet).toHaveCount(0);
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();
  await expect(page.getByRole("dialog", { name: "授权或导入账号" }).locator("textarea[name=secret]")).toHaveValue("");
});

test("chip edits are protected before an egress policy or provider can close", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);

  await navigate(page, "出口策略");
  const policyRow = page.locator("tr", { hasText: "仅中转站" });
  await policyRow.getByRole("button", { name: "编辑", exact: true }).click();
  const policySheet = page.getByRole("dialog", { name: /编辑 仅中转站/ });
  await expect(policySheet).toBeVisible();
  await policySheet.getByRole("button", { name: "移除 relay-a.example.com" }).click();
  await policySheet.getByRole("button", { name: "关闭面板" }).click();
  await expect(page.getByRole("alertdialog", { name: "放弃未保存的修改？" })).toBeVisible();
  await page.getByRole("alertdialog").getByRole("button", { name: "继续编辑" }).click();
  await expect(policySheet.getByRole("button", { name: "移除 relay-a.example.com" })).toHaveCount(0);
  await policySheet.getByRole("button", { name: "取消", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "放弃修改" }).click();

  await navigate(page, "上游");
  const provider = page.locator(".provider-card", { hasText: "中转站 A" });
  await provider.getByRole("button", { name: "编辑", exact: true }).click();
  const providerSheet = page.getByRole("dialog", { name: /编辑 中转站 A/ });
  await expect(providerSheet).toBeVisible();
  await providerSheet.getByRole("button", { name: "移除 minimax-m3" }).click();
  await providerSheet.getByRole("button", { name: "关闭面板" }).click();
  await expect(page.getByRole("alertdialog", { name: "放弃未保存的修改？" })).toBeVisible();
  await page.getByRole("alertdialog").getByRole("button", { name: "放弃修改" }).click();
  await expect(providerSheet).toHaveCount(0);
});
