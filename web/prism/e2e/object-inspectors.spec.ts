import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("configuration objects open read-only inspectors and edit in centered forms", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await unlock(page);
  await selectDraft(page);
  for (const [section, evidence, editLabel] of [
    ["上游", "Provider 家族", "编辑上游"],
    ["模型与路由", "声明能力", "编辑模型"],
    ["访问控制", "访问组 ID", "编辑访问组"],
    ["出口策略", "精确主机", "编辑策略"],
  ]) {
    await navigate(page, section!);
    await page.getByRole("button", { name: "详情", exact: true }).first().click();
    const inspector = page.getByRole("dialog");
    await expect(inspector).toContainText(evidence!);
    await expect(page.locator('.sheet-backdrop[data-layout="inspector"]')).toBeVisible();
    expect((await inspector.boundingBox())?.width).toBeCloseTo(520, 0);
    await inspector.getByRole("button", { name: editLabel!, exact: true }).click();
    const form = page.getByRole("dialog");
    await expect(page.locator('.sheet-backdrop[data-layout="form"]:not(.sheet-ghost)')).toBeVisible();
    expect((await form.boundingBox())?.width).toBeCloseTo(600, 0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }
});

test("Key inspection only shows metadata and respects published-version write locks", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", { name: "发布" }).click();
  await expect(page.getByRole("dialog")).toContainText("已发布");
  await page.getByRole("button", { name: "完成", exact: true }).click();
  await navigate(page, "访问控制");
  const keyTable = page.locator(".card.tablewrap").filter({ hasText: "Client Key(仅前缀" });
  await keyTable.getByRole("button", { name: "详情", exact: true }).first().click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText("只显示公开元数据");
  await expect(inspector.getByRole("button", { name: "编辑 Client Key" })).toBeDisabled();
  expect(await inspector.textContent()).not.toMatch(/rgw_[0-9a-f]{16}_[0-9a-f]{64}/u);
});

test("price inspector preserves all six token rates without inventing currency", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "计费与价格");
  await page.getByRole("button", { name: "详情", exact: true }).first().click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText("全局价格目录");
  await inspector.locator(".price-evidence summary").first().click();
  for (const label of ["输入", "输出", "推理", "缓存读", "缓存写", "已缓存"]) {
    await expect(inspector.getByText(label, { exact: true }).first()).toBeVisible();
  }
  expect(await inspector.textContent()).not.toMatch(/USD|CNY|美元|人民币/u);
});

test("audit and route inspectors expose the observed identity and version", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "审计与备份");
  await page.getByRole("button", { name: "详情", exact: true }).first().click();
  await expect(page.getByRole("dialog")).toContainText("执行者");
  await expect(page.getByRole("dialog")).toContainText("配置版本");
  await page.keyboard.press("Escape");
  await navigate(page, "模型与路由");
  await page.getByRole("button", { name: "建路由", exact: true }).first().click();
  await page.getByRole("dialog").getByLabel("路由 ID").fill("rt-inspector");
  await page.getByRole("dialog").getByRole("button", { name: "创建", exact: true }).click();
  await page.locator(".route-workbench").getByRole("button", { name: "打开它" }).click();
  await page.getByRole("button", { name: "路由详情", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("最大尝试次数");
  await expect(page.getByRole("dialog")).toContainText("rt-inspector");
});

test("long model identities wrap inside the mobile inspector", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "模型与路由");
  const id = `model-${"x".repeat(120)}`;
  await page.getByRole("button", { name: "新建公开模型" }).click();
  const form = page.getByRole("dialog");
  await form.getByLabel("模型 ID").fill(id);
  await form.getByLabel("模型名", { exact: false }).fill(id);
  await form.getByRole("button", { name: "保存", exact: true }).click();
  await page.locator("tr").filter({ hasText: id }).getByRole("button", { name: "详情", exact: true }).click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText(id);
  const dimensions = await inspector.locator(".sheet-panel").evaluate((el) => ({ width: el.clientWidth, content: el.scrollWidth }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.width + 1);
});
