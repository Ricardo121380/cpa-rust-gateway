// 用量分析 over the real contract (GET /admin/operations/usage).
//
// Each assertion here targets a way this page could look right and be wrong:
// a total computed from the first page only, an unobserved token count printed
// as zero, a silent truncation, and a null access group quietly dropped. Unit
// tests cover the arithmetic; these cover the arithmetic actually reaching the
// screen with the paging the contract forces.
import { expect, test } from "@playwright/test";
import { navigate, unlock } from "./helpers";

// No version is selected anywhere in this file on purpose: listOperationalUsage
// declares no X-Config-Version, so the page must work without one. Selecting a
// version here would hide a regression that re-introduced the dependency.
async function openUsage(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await navigate(page, "用量分析");
  await expect(page.locator(".usage-table")).toBeVisible();
}

test("totals follow the cursor instead of stopping at the first page", async ({ page }) => {
  await openUsage(page);

  // The fixture returns 137 rows and `limit` caps at 100, so the page must
  // fetch twice. 1,225 is the sum over all 137; 885 is what a first-page-only
  // sum would produce — the exact failure this test exists to catch.
  await expect(page.locator(".usage-table tfoot")).toContainText("1,225");
  await expect(page.locator(".usage-table tfoot")).not.toContainText("885");
  await expect(page.locator(".usage-watermark")).toContainText("读取 2 页");
  await expect(page.locator(".usage-watermark")).toContainText("已到末页");
});

test("an unobserved token count is a lower bound, not a zero", async ({ page }) => {
  await openUsage(page);

  // Every 11th fixture row reports `total: null`. Those rows must drag their
  // group to "unknown" and mark the figure as a floor.
  const footer = page.locator(".usage-table tfoot");
  await expect(footer).toContainText("≥");
  await expect(footer.locator('.usage-conf[data-tone="muted"]').first()).toContainText("未知");

  // The legend explains the marker rather than leaving it as decoration.
  await expect(page.locator(".usage-page")).toContainText("是下界");
  await expect(page.locator(".usage-page")).toContainText("未观测与零是两件事");
});

test("grouping switches dimension and survives in the URL", async ({ page }) => {
  await openUsage(page);
  await expect(page.locator(".usage-table thead")).toContainText("Provider");

  await page.getByLabel("分组维度").selectOption("public_model");
  await expect(page).toHaveURL(/by=public_model/u);
  await expect(page.locator(".usage-table thead")).toContainText("公开模型");
  await expect(page.locator(".usage-table tbody tr")).toHaveCount(3);

  // Totals are a property of the rows, not of the grouping — regrouping the
  // same rows must not change the sum.
  await expect(page.locator(".usage-table tfoot")).toContainText("1,225");
});

test("a Client Key with no access group gets its own bucket", async ({ page }) => {
  await openUsage(page);
  await page.getByLabel("分组维度").selectOption("access_group_id");
  // Folding these into "" would hide the keys that answer to no group limits.
  await expect(page.locator(".usage-table tbody")).toContainText("(无访问组)");
});

test("truncation is announced, never silent", async ({ page }) => {
  await openUsage(page);

  // prov-flood yields 2,400 rows; the page stops at 20 pages x 100 and must say
  // the totals below are incomplete rather than presenting 2,000 as the answer.
  await page.getByLabel("Provider", { exact: true }).fill("prov-flood");
  await page.getByRole("button", { name: "应用筛选" }).click();

  const warning = page.locator(".action-error");
  await expect(warning).toContainText("被截断");
  await expect(warning).toContainText("下面的合计是不完整的");
});

test("the page offers no trend, heatmap or zoom, and says why", async ({ page }) => {
  await openUsage(page);

  // The contract has no server-side time buckets. A chart here would be
  // invented data, so the absence is deliberate and stated.
  await expect(page.locator(".zoom-brush")).toHaveCount(0);
  await expect(page.locator("svg.chart-svg")).toHaveCount(0);
  await expect(page.locator(".usage-page")).toContainText("契约没有服务端时间桶");
  await expect(page.locator(".usage-page")).toContainText("成本不在本页");
});

test("an unknown protocol in the URL is dropped rather than sent", async ({ page }) => {
  await unlock(page);
  const failed: string[] = [];
  page.on("response", (response) => {
    if (response.url().includes("/admin/operations/usage") && response.status() >= 400) {
      failed.push(response.url());
    }
  });

  await page.goto("/#/usage?protocol=grpc");
  await expect(page.locator(".usage-table")).toBeVisible();
  // Forwarding it would earn a 400 that reads as a panel bug.
  expect(failed).toEqual([]);
});
