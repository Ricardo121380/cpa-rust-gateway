import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("starting an edit copies the active graph without asking for credentials again", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", { name: "发布", exact: true }).click();
  await page.getByRole("dialog", { name: "确认发布" }).getByRole("button", { name: "确认发布", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("已发布");
  await page.getByRole("button", { name: "完成", exact: true }).click();
  await navigate(page, "配置版本");
  await page.getByRole("button", { name: "编辑当前配置", exact: true }).click();
  await expect(page.locator(".configuration-context")).toHaveAttribute("data-status", "draft");
  await expect(page.locator(".configuration-context")).toHaveAttribute("data-context-version", /^edit-/u);
  await navigate(page, "上游");
  const row = page.locator("tr", {hasText: "relay-a"}).first();
  await expect(row).toBeVisible();
  await row.getByRole("button", {name:"子资源", exact:true}).click();
  await expect(page.locator(".subresource-panel")).toContainText("cred-relay-key");
  await expect(page.locator(".subresource-panel")).toContainText("cred-codex-oauth");
  await expect(page.locator(".subresource-panel")).toContainText("ep-relay-a-responses");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
