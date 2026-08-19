// 请求监控 over the two contract sources that actually exist.
//
// The page this replaces claimed latency percentiles and a success rate. The
// assertions below are mostly about what must NOT be on screen, plus the two
// traps that are easy to build wrong: a summary read from the loaded page
// instead of the whole window, and two panels whose scopes silently differ.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openMonitoring(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await navigate(page, "请求监控");
  await expect(page.locator(".mon-table")).toBeVisible();
}

test("the page states there is no latency and no success rate, and shows neither", async ({
  page,
}) => {
  await openMonitoring(page);

  await expect(page.locator(".mon-hint")).toContainText("没有延迟");
  await expect(page.locator(".mon-hint")).toContainText("没有请求成败清单");
  await expect(page.locator(".mon-hint")).toContainText("是编的");

  // The disclaimer names them on purpose; what must not exist is a KPI or a
  // column presenting them as data. So the negative assertions are scoped to
  // where a fabricated metric would actually surface.
  await expect(page.locator(".mon-summary")).not.toContainText("P95");
  await expect(page.locator(".mon-summary")).not.toContainText("成功率");
  await expect(page.locator(".mon-summary")).not.toContainText("延迟");
  await expect(page.locator(".mon-table thead")).not.toContainText("延迟");
  await expect(page.locator(".mon-table thead")).not.toContainText("状态");
});

test("the ledger summary covers the whole window, not the loaded page", async ({ page }) => {
  await openMonitoring(page);

  // 73 fixture rows; the first page holds 100 so everything is loaded here, but
  // the figures come from the backend's own summary either way. 41.1% exact and
  // 200,340 microunits are properties of the full filtered set.
  await expect(page.locator(".mon-summary")).toContainText("73");
  await expect(page.locator(".mon-summary")).toContainText("41.1%");
  await expect(page.locator(".mon-summary")).toContainText("200,340");
  await expect(page.locator(".mon-summary")).toContainText("覆盖整个筛选窗口");
});

test("cost is microunits with no currency invented", async ({ page }) => {
  await openMonitoring(page);
  await expect(page.locator(".mon-summary")).toContainText("microunits");
  await expect(page.locator(".mon-table")).not.toContainText("$");
  await expect(page.locator(".mon-table")).not.toContainText("¥");
});

test("the confidence filter is labelled as pricing, never as request status", async ({ page }) => {
  await openMonitoring(page);
  // The contract calls this parameter `status`, which invites exactly the wrong
  // reading; the control must not repeat the mistake.
  await expect(page.locator(".mon-filters")).toContainText("计价置信度");
  await expect(page.locator(".mon-filters")).not.toContainText("状态");

  await page.getByLabel("计价置信度").selectOption("unpriced");
  await page.getByRole("button", { name: "应用筛选" }).click();
  await expect(page.locator(".mon-summary")).toContainText("14");
});

test("a ledger row drills into its attempt trail", async ({ page }) => {
  await openMonitoring(page);
  await page.locator(".mon-table tbody .linklike").first().click();

  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("裸数组");
  // `outcome` is a free string in the contract, so it is shown verbatim rather
  // than mapped into a closed vocabulary the backend never promised.
  await expect(sheet).toContainText("provider_rate_limited");
  await expect(sheet.locator(".mon-attempts tbody tr")).toHaveCount(2);
});

test("the failure panel is version-scoped and says so when the ledger is not", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");
  await page.getByRole("tab", { name: "失败归因" }).click();

  // No version selected: the ledger next door works fine without one, and an
  // operator needs to be told why this panel does not.
  await expect(page.locator(".empty-state")).toContainText("需要一个配置版本");
  await expect(page.locator(".empty-state")).toContainText("X-Config-Version");

  await selectDraft(page);
  await expect(page.locator(".mon-table")).toBeVisible();
});

test("failure counts are labelled as loaded-so-far, and rows are not requests", async ({
  page,
}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "请求监控");
  await page.getByRole("tab", { name: "失败归因" }).click();
  await expect(page.locator(".mon-table")).toBeVisible();

  await expect(page.locator(".mon-summary")).toContainText("已加载的这些行");
  // One request can fail several times; presenting the row count as a failed
  // request count would overstate it.
  await expect(page.locator(".mon-summary")).toContainText("不是失败请求数");
  await expect(page.locator(".mon-breakdown")).toContainText("ProviderRateLimited");
});

test("the ledger exports parseable JSONL that the production CSP does not block", async ({
  page,
}) => {
  // A blocked download and an unparseable file are both invisible to unit
  // tests, which is the only reason this one drives a browser.
  await openMonitoring(page);

  const button = page.getByRole("button", { name: /导出 JSONL/u });
  await expect(button).toContainText("73 行");

  const download = await Promise.all([page.waitForEvent("download"), button.click()]).then(
    ([event]) => event,
  );
  expect(download.suggestedFilename()).toMatch(/^prism-billing-[\d-]+\.jsonl$/u);

  const stream = await download.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString("utf8");
  const lines = text.trimEnd().split("\n");

  expect(lines).toHaveLength(74); // header + 73 rows
  for (const line of lines) {
    expect(() => JSON.parse(line) as unknown).not.toThrow();
  }
  const header = JSON.parse(lines[0] as string) as Record<string, unknown>;
  expect(header["format"]).toBe("prism.billing-ledger.v1");
  expect(header["partial"]).toBe(false);
  // Value-free: no body ever reaches the file, because none exists upstream.
  expect(text).not.toMatch(/"(body|request_body|prompt|messages|content)"/u);
});
