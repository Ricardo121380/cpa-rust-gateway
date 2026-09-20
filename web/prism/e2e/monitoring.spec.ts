// Request terminal metrics, ledger totals and failure attempts have distinct scopes.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock, clearVersionForTest } from "./helpers";

async function openMonitoring(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await navigate(page, "请求与失败");
  await page.getByRole("tab",{name:"计费账本"}).click();
  await expect(page.locator(".mon-table")).toBeVisible();
}

test("request metrics use terminal requests and keep attempts separate", async ({page}) => {
  await unlock(page);
  await navigate(page,"请求与失败");
  const metrics=page.locator(".request-metrics");
  await expect(metrics.getByRole("link",{name:"请求数 4",exact:true})).toBeVisible();
  await expect(metrics.getByRole("link",{name:"成功率 75.0%",exact:true})).toBeVisible();
  await expect(metrics).toContainText("P50 / P95");
  await expect(page.locator(".request-history")).toContainText("5 次上游尝试");
  await expect(page.locator(".request-table tbody tr")).toHaveCount(4);
  await metrics.getByRole("link",{name:"成功率 75.0%",exact:true}).click();
  await expect(page).toHaveURL(/outcome=succeeded/u);
  await expect(page.locator(".request-table tbody tr")).toHaveCount(3);
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
  await expect(sheet).toContainText("跨配置版本读取");
  // `outcome` is a free string in the contract, so it is shown verbatim rather
  // than mapped into a closed vocabulary the backend never promised.
  await expect(sheet).toContainText("provider_rate_limited");
  await expect(sheet.locator(".mon-attempts tbody tr")).toHaveCount(2);
});

test("the failure panel is version-scoped and says so when the ledger is not", async ({ page }) => {
  await unlock(page);
  await clearVersionForTest(page);
  await navigate(page, "请求与失败");
  await page.getByRole("tab", { name: "失败归因" }).click();

  // No version selected: the ledger next door works fine without one, and an
  // operator needs to be told why this panel does not.
  await expect(page.locator(".empty-state")).toContainText("需要一个配置版本");
  await expect(page.locator(".empty-state")).toContainText("X-Config-Version");

  await selectDraft(page);
  await page.getByRole("tab",{name:"失败归因"}).click();
  await expect(page.locator(".mon-table")).toBeVisible();
});

test("failure counts are labelled as loaded-so-far, and rows are not requests", async ({
  page,
}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "请求与失败");
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

test("a failure opens request attempts and its exact diagnostic target, preserving filters", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "请求与失败");
  await page.getByRole("tab", { name: "失败归因" }).click();
  // The tab replaces the ledger's form. Wait for that committed view before
  // filling a field whose accessible name also exists on the departing form.
  await expect(page.getByRole("tab", { name: "失败归因" })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("combobox", { name: "账号", exact: true }).selectOption({label:"指定历史资源…"});
  await page.getByRole("textbox",{name:"历史资源引用"}).fill("acct-0");
  await page.getByRole("button",{name:"使用此引用"}).click();
  await page.getByRole("button", { name: "应用筛选", exact: true }).click();
  await expect(page).toHaveURL(/account_id=acct-0/u);
  const source = page.url();
  const row = page.locator(".mon-table tbody tr").first();
  await row.getByRole("button").click();
  await expect(page.getByRole("dialog")).toContainText("上游尝试记录");
  await page.keyboard.press("Escape");
  await expect(row.getByRole("button")).toBeFocused();
  const link = row.getByRole("link", { name: "诊断此绑定" });
  const href = await link.getAttribute("href");
  await link.click();
  await expect(page).toHaveURL(/credential_id=acct-0/u);
  await expect(page.getByRole("region", { name: "当前诊断对象" }).locator('[data-resource-id="acct-0"]')).toBeVisible();
  expect(href).toContain("endpoint_id=ch-relay-responses");
  await page.goBack();
  await expect(page).toHaveURL(source);
  await expect(page.getByRole("combobox", { name: "账号", exact: true })).toHaveValue("acct-0");
});
