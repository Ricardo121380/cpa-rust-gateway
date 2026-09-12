import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("configuration diff selects a baseline, stops stale pages and restarts explicitly", async ({ page }) => {
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
  await page.locator('tr[data-version-id="draft-2026-08"]').first().getByRole("button", { name: "查看差异" }).click();
  const dialog = page.getByRole("dialog", { name: "配置资源差异" });
  await expect(dialog).toContainText("已载入 50 项差异");
  await expect(dialog).toContainText("仅展示变化字段名");
  await page.evaluate(async () => {
    const path = "/src/api/client.ts";
    const { call } = await import(path);
    await call("createPublicModel", { body: { id: "diff-late", model_name: "diff-late", status: "disabled", display_name: "late", capabilities: {} } }, { versionScoped: true, mutating: true });
  });
  await dialog.getByRole("button", { name: "加载更多差异" }).click();
  await expect(dialog).toContainText("分页已停止");
  await expect(dialog.getByRole("button", { name: "加载更多差异" })).toBeDisabled();
  await dialog.getByRole("button", { name: "重新比较" }).click();
  await expect(dialog.getByRole("button", { name: "加载更多差异" })).toBeEnabled();
  await dialog.getByRole("button", { name: "加载更多差异" }).click();
  await expect(dialog).toContainText("diff-late");
  await dialog.getByLabel("比较基线").selectOption("draft-2026-08");
  await expect(dialog).toContainText("两个版本的资源记录没有差异");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(":focus")).toHaveText("查看差异");
});
