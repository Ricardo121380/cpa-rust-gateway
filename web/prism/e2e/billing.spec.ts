// 计费与价格目录 over the six real billing operations.
//
// The assertions target the two scope confusions this page exists to prevent
// (catalogs are global, the policy is per version), the forward-only nature of
// import and rollback, and the future-catalog binding the backend refuses.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openBilling(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "计费与价格");
  await expect(page.locator(".bill-table")).toBeVisible();
}

test("an unset policy reads as a state, not as an error", async ({ page }) => {
  await openBilling(page);

  // 404 management_resource_not_found is the contract's answer for "not
  // configured", and it is why every candidate's price_evidence is `disabled`.
  await expect(page.locator(".bill-policy")).toContainText("本版本未配置价格策略");
  await expect(page.locator(".bill-policy")).toContainText("正常状态");
  await expect(page.locator(".bill-policy .action-error")).toHaveCount(0);
  await expect(page.locator(".bill-policy")).toContainText("disabled");
});

test("the policy picker refuses a catalog that is not effective yet", async ({ page }) => {
  await openBilling(page);

  // cat-2026-09-preview is dated ahead of the fixture clock. The backend fails
  // such a binding with RoutingPriceCatalogNotEffective, so it must not be
  // offered at all.
  await expect(page.locator(".bill-table")).toContainText("cat-2026-09-preview");
  await expect(page.locator(".bill-table")).toContainText("未生效");

  await page.getByRole("button", { name: "设置策略" }).click();
  const options = await page.getByLabel("目录版本").locator("option").allTextContents();
  expect(options.join(" ")).toContain("cat-2026-08");
  expect(options.join(" ")).not.toContain("cat-2026-09-preview");
});

test("binding a catalog, then clearing it, states what each does to routing", async ({ page }) => {
  await openBilling(page);

  await page.getByRole("button", { name: "设置策略" }).click();
  await page.getByLabel("目录版本").selectOption("cat-2026-08");
  await page.getByRole("button", { name: "绑定" }).click();

  await expect(page.locator(".bill-policy")).toContainText("cat-2026-08");
  await expect(page.locator(".bill-policy")).toContainText("rate_dominance_v1");

  await page.getByRole("button", { name: "清除策略" }).click();
  const sheet = page.getByRole("dialog");
  // The consequence is named, not implied: every candidate's evidence changes.
  await expect(sheet).toContainText("price_evidence");
  await expect(sheet).toContainText("disabled");
  await page.getByRole("button", { name: "确认清除" }).click();
  await expect(page.locator(".bill-policy")).toContainText("本版本未配置价格策略");
});

test("the page says catalogs are global while the policy is version-scoped", async ({ page }) => {
  await openBilling(page);
  // The single most likely operator misreading: "I am on a draft, so this
  // import is isolated". It is not.
  await expect(page.locator(".bill-catalogs")).toContainText("目录是全局");
  await expect(page.locator(".bill-catalogs")).toContainText("对所有版本可见");
  await expect(page.locator(".bill-policy")).toContainText("属于当前配置版本");
});

test("import is whole-catalog and insert-only, and says so", async ({ page }) => {
  await openBilling(page);
  await page.getByRole("button", { name: "导入目录" }).click();

  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("整份提交");
  await expect(sheet).toContainText("只能新增");
  await expect(sheet).toContainText("所有配置版本");
});

test("a bad entry is rejected by row number, before the round trip", async ({ page }) => {
  await openBilling(page);
  const failed: string[] = [];
  page.on("response", (response) => {
    if (response.url().includes("/admin/billing/catalogs") && response.request().method() === "POST") {
      failed.push(String(response.status()));
    }
  });

  await page.getByRole("button", { name: "导入目录" }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("目录版本 ID").fill("cat-bad");
  await sheet.getByLabel("生效时间", { exact: false }).fill("2026-08-01T00:00");
  await sheet
    .getByLabel("条目", { exact: false })
    .fill(
      '[{"provider_id":"p","channel_id":"c","model":"m","input_microunits_per_million":1,' +
        '"output_microunits_per_million":1,"reasoning_microunits_per_million":1,' +
        '"cache_read_microunits_per_million":1,"cache_creation_microunits_per_million":1,' +
        '"cached_microunits_per_million":1},' +
        '{"provider_id":"p","channel_id":"c","model":"","input_microunits_per_million":1,' +
        '"output_microunits_per_million":1,"reasoning_microunits_per_million":1,' +
        '"cache_read_microunits_per_million":1,"cache_creation_microunits_per_million":1,' +
        '"cached_microunits_per_million":1}]',
    );
  await sheet.getByRole("button", { name: "导入" }).click();

  await expect(page.locator(".action-error")).toContainText("第 2 条");
  await expect(page.locator(".action-error")).toContainText("model");
  // A 400 on a 512-row paste is useless, so the request is never sent.
  expect(failed).toEqual([]);
});

test("rollback creates a new catalog rather than deleting anything", async ({ page }) => {
  await openBilling(page);
  const before = await page.locator(".bill-table tbody tr").count();

  await page
    .locator("tr", { hasText: "cat-2026-07" })
    .getByRole("button", { name: "回滚到它" })
    .click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("不会删除任何东西");
  await sheet.getByLabel("新目录版本 ID").fill("cat-restored");
  await sheet.getByLabel("生效时间", { exact: false }).fill("2026-08-18T00:00");
  await sheet.getByRole("button", { name: "创建回滚目录" }).click();

  await expect(page.locator(".bill-table tbody tr")).toHaveCount(before + 1);
  await expect(page.locator(".bill-table")).toContainText("cat-2026-07");
  await expect(page.locator(".action-notice")).toContainText("复制自 cat-2026-07");
  // Importing does not bind: the operator still has to bind it explicitly.
  await expect(page.locator(".action-notice")).toContainText("还需在上方绑定");
});

test("rates render as microunits with no currency anywhere", async ({ page }) => {
  await openBilling(page);
  await page
    .locator("tr", { hasText: "cat-2026-08" })
    .getByRole("button", { name: "看条目" })
    .click();

  await expect(page.locator(".bill-entries-view")).toContainText("microunits / 百万 token");
  await expect(page.locator(".bill-entries-view")).toContainText("1,500,000");
  await expect(page.locator(".billing-page")).not.toContainText("$");
  await expect(page.locator(".billing-page")).not.toContainText("¥");
});
