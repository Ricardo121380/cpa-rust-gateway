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
  await page.getByText("高级路由价格策略",{exact:true}).click();
}

test("an unset policy reads as a state, not as an error", async ({ page }) => {
  await openBilling(page);

  // 404 management_resource_not_found is the contract's answer for "not
  // configured", and it is why every candidate's price_evidence is `disabled`.
  await expect(page.locator(".bill-policy")).toContainText("当前版本未配置价格策略");
  await expect(page.locator(".bill-policy")).toContainText("正常的未配置状态");
  await expect(page.locator(".bill-policy .action-error")).toHaveCount(0);
  await expect(page.locator(".bill-policy")).toContainText("disabled");
});

test("the policy picker refuses a catalog that is not effective yet", async ({ page }) => {
  await openBilling(page);

  // cat-2026-09-preview is dated ahead of the fixture clock. The backend fails
  // such a binding with RoutingPriceCatalogNotEffective, so it must not be
  // offered at all.
  await expect(page.locator('[data-resource-id="cat-2026-09-preview"]')).toBeVisible();
  await expect(page.locator(".bill-table")).toContainText("未生效");

  await page.getByRole("button", { name: "设置策略" }).click();
  const options=page.getByRole("dialog").getByRole("combobox",{name:"已生效的价格目录"});
  await expect(options.locator('option[value="cat-2026-08"]')).toHaveCount(1);
  await expect(options.locator('option[value="cat-2026-09-preview"]')).toHaveCount(0);
});

test("binding a catalog, then clearing it, states what each does to routing", async ({ page }) => {
  await openBilling(page);

  await page.getByRole("button", { name: "设置策略" }).click();
  await page.getByRole("dialog").getByRole("combobox",{name:"已生效的价格目录"}).selectOption("cat-2026-08");
  await page.getByRole("dialog").getByRole("button", { name: "保存到草稿" }).click();
  await page.getByRole("dialog",{name:"价格策略结果"}).getByRole("button",{name:"完成"}).click();

  await expect(page.locator(".bill-policy")).toContainText("按费率比较");
  const bound=await page.evaluate(async()=>{const {call}=await import("/src/api/client.ts");return await call<{catalog_version_id:string}>("getRoutingPricePolicy",{},{versionScoped:true});});
  expect(bound.catalog_version_id).toBe("cat-2026-08");

  await page.getByRole("button", { name: "清除策略" }).click();
  const sheet = page.getByRole("dialog");
  // The consequence is named, not implied: every candidate's evidence changes.
  await expect(sheet).toContainText("目录和账本保留");
  await sheet.getByRole("button", { name: "确认清除" }).click();
  await page.getByRole("dialog",{name:"价格策略结果"}).getByRole("button",{name:"完成"}).click();
  await expect(page.locator(".bill-policy")).toContainText("当前版本未配置价格策略");
});

test("the page says catalogs are global while the policy is version-scoped", async ({ page }) => {
  await openBilling(page);
  // The single most likely operator misreading: "I am on a draft, so this
  // import is isolated". It is not.
  await expect(page.locator(".bill-catalogs")).toContainText("全局新增历史版本");
  await expect(page.locator(".bill-catalogs")).toContainText("对所有配置可见");
  await expect(page.locator(".bill-catalog-table")).not.toContainText("cat-2026-09-preview");
  await expect(page.locator(".bill-policy")).toContainText("只属于工作配置草稿");
});

test("import is whole-catalog and insert-only, and says so", async ({ page }) => {
  await openBilling(page);
  await page.getByRole("button", { name: "导入目录" }).click();

  const sheet = page.locator(".inline-workspace");
  await expect(sheet).toContainText("整份价目表");
  await expect(sheet).toContainText("历史目录和账本保留");
  await expect(sheet).toContainText("所有配置可见");
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
  const sheet = page.locator(".inline-workspace");
  await sheet.getByLabel("生效时间", { exact: false }).fill("2026-08-01T00:00");
  await sheet.getByRole("button",{name:"高级 JSON"}).click();
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
  await sheet.getByRole("button", { name: "预览差异" }).click();

  await expect(sheet.getByRole("alert")).toContainText("第 2 条");
  await expect(sheet.getByRole("alert")).toContainText("model");
  // A 400 on a 512-row paste is useless, so the request is never sent.
  expect(failed).toEqual([]);
});

test("rollback creates a new catalog rather than deleting anything", async ({ page }) => {
  await openBilling(page);
  const before = await page.locator(".bill-table tbody tr").count();

  await page
    .locator('[data-resource-id="cat-2026-07"]')
    .getByRole("button", { name: "恢复价格" })
    .click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("旧目录与账本原样保留");
  await sheet.getByText("新目录技术标识",{exact:true}).click();
  await sheet.getByLabel("新目录版本 ID").fill("cat-restored");
  await sheet.getByLabel("生效时间", { exact: false }).fill("2026-08-18T00:00");
  await sheet.getByRole("button", { name: "核对恢复内容" }).click();
  await page.getByRole("dialog",{name:"确认创建恢复目录"}).getByRole("button",{name:"创建新目录"}).click();
  await page.getByRole("dialog",{name:"价格恢复结果"}).getByRole("button",{name:"完成"}).click();

  await expect(page.locator(".bill-table tbody tr")).toHaveCount(before + 1);
  await expect(page.locator('[data-resource-id="cat-2026-07"]')).toBeVisible();
  // Restoring is append-only; the routing policy is still a separate draft action.
  await expect(page.locator(".bill-policy")).toContainText("未配置价格策略");
});

test("rates render as microunits with no currency anywhere", async ({ page }) => {
  await openBilling(page);
  await page
    .locator('[data-resource-id="cat-2026-08"]')
    .getByRole("button", { name: "看条目" })
    .click();

  await page.locator(".bill-entries-view details").first().locator("summary").click();

  await expect(page.locator(".bill-entries-view")).toContainText("microunits / 百万 token");
  await expect(page.locator(".bill-entries-view")).toContainText("1,500,000");
  await expect(page.locator(".billing-page")).not.toContainText("$");
  await expect(page.locator(".billing-page")).not.toContainText("¥");
});
