import { expect, test } from "@playwright/test";
import { unlock, navigate } from "./helpers";

test("serving model contexts stay separate and source links preserve exact IDs", async ({ page }) => {
  await unlock(page);
  await page.locator(".version-picker select").selectOption("v-2026-07");
  await navigate(page, "模型目录");
  const panel = page.getByRole("region", { name: "授权有效模型" });
  await panel.getByLabel("模型授权身份").selectOption("team-default");
  await expect(panel).toContainText("exact-alpha");
  await panel.getByLabel("模型授权身份").selectOption("team-batch");
  await expect(panel).toContainText("exact-beta");
  await expect(panel).not.toContainText("exact-alpha");
  await panel.getByRole("button", { name: "模型来源" }).click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText("endpoint-exact-beta");
  await expect(inspector).toContainText("目录 v7");
  await expect(inspector).toContainText("硬过期");
  await expect(inspector).toContainText("目录准入有效");
  await inspector.getByRole("link", { name: "在诊断中检查该模型" }).click();
  await expect(page.getByLabel("请求模型", { exact: true })).toHaveValue("exact-beta");
  await navigate(page, "模型目录");
  await panel.getByLabel("模型授权上下文类型").selectOption("client_key_id");
  await panel.getByLabel("模型授权身份").selectOption("key-cli");
  await expect(panel).toContainText("exact-alpha");
  await panel.getByLabel("模型授权身份").selectOption("key-dead");
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel).not.toContainText("exact-alpha");
});
