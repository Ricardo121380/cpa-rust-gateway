import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("starting an edit copies the active graph without asking for credentials again", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", { name: "查看变更", exact: true }).click();
  await page.getByRole("region", {name:"待应用变更",exact:true}).getByRole("button", {name:"校验并应用",exact:true}).click();
  await page.getByRole("dialog", { name: "确认应用配置" }).getByRole("button", { name: "确认应用", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("已确认");
  await page.getByRole("button", { name: "完成", exact: true }).click();
  await navigate(page, "配置版本");
  await page.getByRole("button", { name: "编辑当前配置", exact: true }).click();
  await page.getByRole("dialog",{name:"编辑当前配置"}).getByRole("button",{name:"创建草稿",exact:true}).click();
  await page.getByRole("dialog",{name:"草稿创建结果"}).getByRole("button",{name:"接续草稿",exact:true}).click();
  await expect(page.locator("main.canvas")).toHaveAttribute("data-context-status", "draft");
  await expect(page.locator("main.canvas")).toHaveAttribute("data-context-version", /^edit-/u);
  await navigate(page, "上游");
  const provider = page.locator("article", {hasText: "中转站 A"});
  await expect(provider).toBeVisible();
  await provider.getByRole("button", {name:"接口与账号", exact:true}).click();
  await expect(page.locator(".subresource-panel")).toContainText("Responses");
  await expect(page.locator(".subresource-panel")).toContainText("alex@example.test");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
