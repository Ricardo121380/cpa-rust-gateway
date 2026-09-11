import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

const WORKSPACES = ["仪表盘", "请求日志", "用量与费用", "计费与价格", "账号管理", "AI 提供商", "模型目录", "模型管理", "API 密钥", "运行诊断", "出口策略", "配置版本", "审计与备份", "设置"];

for (const [width, height] of [[1440, 900], [1280, 720], [390, 844]]) {
  test(`all 14 workspaces and unlock at ${width}×${height}`, async ({ page }, info) => {
    await page.setViewportSize({ width: width!, height: height! });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await unlock(page);
    await selectDraft(page);
    for (const name of WORKSPACES) {
      await navigate(page, name);
      await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
      expect(await page.locator(".glass[data-pane]").count()).toBeLessThanOrEqual(3);
      const workspace = await page.locator(".workspace").boundingBox();
      expect(workspace?.x).toBeGreaterThanOrEqual(0);
      expect((workspace?.x ?? 0) + (workspace?.width ?? 0)).toBeLessThanOrEqual(width!);
      await page.screenshot({ path: info.outputPath(`${name}.png`) });
    }
    expect(errors).toEqual([]);
  });
}

test("account filtering, evidence, failure deep link and focus restoration", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await page.getByRole("textbox", { name: "搜索已加载账号" }).fill("cred-grok-oauth");
  const detail = page.locator(".account-desktop").getByRole("button", { name: "详情" });
  await detail.click();
  const inspector = page.getByRole("dialog");
  const box = await inspector.boundingBox();
  expect(box?.width).toBeCloseTo(520, 0);
  expect(box?.x).toBeCloseTo(904, 0);
  await inspector.getByRole("button", { name: "权益证据" }).click();
  await expect(inspector.getByText("provider_subscription", { exact: true })).toBeVisible();
  await expect(inspector).toContainText("authoritative");
  await page.keyboard.press("Escape");
  await expect(detail).toBeFocused();
  await detail.click();
  await inspector.getByRole("button", { name: "配置与失败" }).click();
  await inspector.getByRole("link", { name: "失败记录" }).click();
  await expect(page).toHaveURL(/tab=failures/u);
  await expect(page.getByRole("tab", { name: "失败归因" })).toHaveAttribute("aria-selected", "true");
  await page.goBack();
  await expect(page.getByRole("textbox", { name: "搜索已加载账号" })).toHaveValue("cred-grok-oauth");
});

test("catalog missing is failure-only, not a zero-model successful discovery", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "模型目录");
  await page.getByRole("combobox", { name: "目录状态" }).selectOption("missing");
  await page.getByRole("button", { name: "详情" }).click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText("authentication");
  await expect(inspector.locator(".fact-grid > div").filter({ hasText: "目录模型数" })).toContainText("未观测");
  await expect(inspector.locator(".fact-grid > div").filter({ hasText: "目录快照" })).toContainText("未观测");
});

test("mobile inspector has 12px margins; dark and accessibility controls keep solid data", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await unlock(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await page.locator(".account-mobile").getByRole("button", { name: "详情" }).first().click();
  const box = await page.getByRole("dialog").boundingBox();
  expect(box?.x).toBeCloseTo(12, 0);
  expect(box?.width).toBeCloseTo(366, 0);
  await page.keyboard.press("Escape");
  await navigate(page, "设置");
  await page.getByRole("radio", { name: "深色", exact: true }).click();
  await page.getByRole("checkbox", { name: "减少透明度" }).check();
  await page.getByRole("checkbox", { name: "增强对比度" }).check();
  await page.getByRole("checkbox", { name: "减少动态效果" }).check();
  expect(await page.locator(".topbar").evaluate((el) => getComputedStyle(el).backdropFilter)).toBe("none");
  expect(await page.locator("html").evaluate((el) => getComputedStyle(el).getPropertyValue("--dur-anneal").trim())).toBe("0ms");
});
