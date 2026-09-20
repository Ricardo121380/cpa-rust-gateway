import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("pending diff stops stale pages and explicitly rereads current revisions", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async () => {
    const path = "/src/api/client.ts";
    const { call } = await import(path);
    for (let index = 0; index < 60; index++) await call("createPublicModel", { body: {
      id: `diff-${index}`, model_name: `diff-${index}`, status: "disabled", display_name: `diff-${index}`, capabilities: {},
    } }, { versionScoped: true, mutating: true });
  });
  await navigate(page, "配置版本");
  await page.locator('tr[data-version-id="draft-2026-08"]').first().getByRole("button", { name: "查看变更" }).click();
  const dialog = page.getByRole("region", { name: "待应用变更", exact:true });
  await expect(dialog).toContainText("已载入部分变化：50 项");
  await expect(dialog.getByRole("button",{name:"校验并应用",exact:true})).toBeDisabled();
  await page.evaluate(async () => {
    const path = "/src/api/client.ts";
    const { call } = await import(path);
    await call("createPublicModel", { body: { id: "diff-late", model_name: "diff-late", status: "disabled", display_name: "late", capabilities: {} } }, { versionScoped: true, mutating: true });
  });
  await dialog.getByRole("button", { name: "加载下一页差异" }).click();
  await expect(dialog).toContainText("差异读取已停止");
  await expect(dialog.getByRole("button", { name: "加载下一页差异" })).toBeDisabled();
  await dialog.getByRole("button", { name: "重新比较" }).click();
  await expect(dialog.getByRole("button", { name: "加载下一页差异" })).toBeEnabled();
  await dialog.getByRole("button", { name: "加载下一页差异" }).click();
  await expect(dialog).toContainText("完整净变化");
  await dialog.getByRole("button",{name:"下一页",exact:true}).click();
  await expect(dialog).toContainText("diff-late");
  await dialog.getByRole("button",{name:"返回配置",exact:true}).click();
  await expect(dialog).toHaveCount(0);
});
