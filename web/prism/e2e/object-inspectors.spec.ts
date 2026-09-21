import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("configuration inspectors lead to the appropriate guarded editor", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await unlock(page);
  await selectDraft(page);
  for (const [section, evidence, editLabel] of [
    ["上游", "Provider 家族", "编辑提供商"],
    ["模型与路由", "声明能力", "编辑模型"],
    ["访问控制", "访问组", "编辑访问组"],
    ["出口策略", "精确主机", "编辑策略"],
  ]) {
    await navigate(page, section!);
    if (section === "上游") await page.locator(".provider-card .row-menu summary").first().click();
    if (section === "访问控制") await page.getByText("高级访问组", { exact: true }).click();
    await page.getByRole("button", { name: "详情", exact: true }).first().click();
    const inspector = page.getByRole("dialog");
    await expect(inspector).toContainText(evidence!);
    await expect(page.locator('.sheet-backdrop[data-layout="inspector"]')).toBeVisible();
    expect((await inspector.boundingBox())?.width).toBeCloseTo(520, 0);
    await inspector.getByRole("button", { name: editLabel!, exact: true }).click();
    if (section === "出口策略") {
      await expect(page.locator(".inline-workspace")).toBeVisible();
      await expect(page.getByRole("dialog")).toHaveCount(0);
      await page.locator(".inline-workspace").getByRole("button", { name: "取消", exact: true }).click();
    } else {
      await expect(page.locator('.sheet-backdrop[data-layout="form"]')).toBeVisible();
      const box = await page.getByRole("dialog").boundingBox();
      expect(box).not.toBeNull();
      expect(box!.x + box!.width / 2).toBeCloseTo(720, 0);
      await page.keyboard.press("Escape");
    }
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }
});

test("Key inspection shows metadata and opens guarded permissions on the active configuration", async ({ page }) => {
  await unlock(page);
  await navigate(page, "访问控制");
  const keyTable = page.locator("table").filter({ hasText: "rgw_9f3c21ab04d7e6b2" });
  await keyTable.getByRole("button", { name: "详情", exact: true }).first().click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText("只显示公开元数据");
  await expect(inspector.getByRole("button", { name: "编辑 Client Key" })).toBeEnabled();
  expect(await inspector.textContent()).not.toMatch(/rgw_[0-9a-f]{16}_[0-9a-f]{64}/u);
});

test("price inspector preserves all six token rates without inventing currency", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "计费与价格");
  await page.getByRole("button", { name: "详情", exact: true }).first().click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toHaveAccessibleName("价格目录详情");
  await expect(inspector).toContainText("全局、只读的完整目录");
  await inspector.locator(".price-preview-list summary").first().click();
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
  await page.locator(".models-inventory .row-menu summary").first().click();
  await page.getByRole("button", { name: "配置路由", exact: true }).first().click();
  await page.getByRole("dialog").getByLabel("路由标识").fill("rt-inspector");
  await page.getByRole("dialog").getByRole("button", { name: "创建路由", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "继续配置候选", exact: true }).click();
  await page.locator(".route-workbench").getByRole("button", { name: "打开它", exact: true }).click();
  await page.getByRole("button", { name: "路由详情", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("最大尝试次数");
  await expect(page.getByRole("dialog")).toContainText("路由");
});

test("long model identities wrap inside the mobile inspector", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "模型与路由");
  const id = `model-${"x".repeat(120)}`;
  await page.getByText("高级路由、候选与别名", { exact: true }).click();
  await page.getByRole("button", { name: "高级模型配置" }).click();
  const form = page.getByRole("dialog");
  await form.getByLabel("模型 ID").fill(id);
  await form.getByLabel("客户端模型名", { exact: true }).fill(id);
  await form.getByRole("button", { name: "保存", exact: true }).click();
  await page.getByRole("dialog", { name: "模型配置结果" }).getByRole("button", { name: "完成", exact: true }).click();
  await page.locator("tr").filter({ hasText: id }).getByRole("button", { name: "详情", exact: true }).click();
  const inspector = page.getByRole("dialog");
  await expect(inspector).toContainText(id);
  const dimensions = await inspector.locator(".sheet-panel").evaluate((el) => ({ width: el.clientWidth, content: el.scrollWidth }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.width + 1);
});
