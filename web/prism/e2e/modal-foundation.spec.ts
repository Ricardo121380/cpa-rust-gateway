import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("advanced group creation retains its sheet until the held write returns", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
  const groups = page.locator("details", { hasText: "高级访问组" });
  await groups.locator("summary").click();
  await groups.getByRole("button", { name: "新建访问组" }).click();
  const sheet = page.getByRole("dialog", { name: "新建访问组" });
  await sheet.getByLabel("访问组标识").fill("pending-group");
  await sheet.getByLabel("名称", { exact: true }).fill("待保存组");
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest("POST /admin/access-groups");
  });
  await sheet.getByRole("button", { name: "创建", exact: true }).click();
  await expect.poll(() => page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest("POST /admin/access-groups");
  })).toBe(1);
  await expect(sheet.getByRole("button", { name: "关闭面板" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await sheet.getByRole("button", { name: "取消", exact: true }).click();
  await expect(sheet).toBeVisible();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest("POST /admin/access-groups");
  });
  await expect(sheet).toHaveCount(0);
  await expect(groups.locator('[data-resource-id="pending-group"]')).toBeVisible();
});

test("proxy deletion cannot hide its pending write with cancel or Escape", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "出口策略");
  const row = page.locator('.cp-section tr:has([data-resource-id="pool-empty"])');
  await row.getByRole("button", { name: "删除", exact: true }).click();
  const sheet = page.getByRole("dialog");
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest("DELETE /admin/compatible-proxy-pools/pool-empty");
  });
  await sheet.getByRole("button", { name: "确认删除" }).click();
  await expect.poll(() => page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest("DELETE /admin/compatible-proxy-pools/pool-empty");
  })).toBe(1);
  await expect(sheet.getByRole("button", { name: "关闭面板" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await sheet.getByRole("button", { name: "取消", exact: true }).click();
  await expect(sheet).toBeVisible();
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest("DELETE /admin/compatible-proxy-pools/pool-empty");
  });
  await expect(sheet).toHaveCount(0);
  await expect(row).toHaveCount(0);
});

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
