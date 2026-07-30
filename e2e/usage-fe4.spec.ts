// FE-4: the sixth usage tab, entity comparison, and JSONL export. Each of these
// has a failure mode that unit tests cannot reach — a rank tab that silently
// returns the wrong dimension, four comparison series that render identically,
// and a download that the production CSP blocks.
import { expect, test } from "@playwright/test";
import { navigate, unlock } from "./helpers";

test("usage has six tabs and Client Key ranks its own dimension", async ({ page }) => {
  await unlock(page);
  await navigate(page, "用量分析");

  const tabs = page.locator(".usage-tabs button");
  await expect(tabs).toHaveCount(6);
  await expect(tabs.nth(3)).toHaveText("Client Key");

  await tabs.nth(3).click();
  await expect(page).toHaveURL(/tab=clientKeys/u);
  await expect(page.getByRole("heading", { name: /Client Key 排行/u })).toBeVisible();

  // the rows must be client keys, not the models the fixture used to return for
  // every dimension. Located by column header rather than by index: the expand
  // column shifts every position, and a positional selector would silently
  // assert on the wrong cell after any column change.
  const table = page.locator(".card.tablewrap table");
  const keyColumn = await table
    .locator("thead th")
    .evaluateAll((headers) => headers.findIndex((h) => h.textContent === "Client Key") + 1);
  expect(keyColumn).toBeGreaterThan(0);
  await expect(table.locator(`tbody tr td:nth-child(${keyColumn})`).first()).toHaveText(/^key-/u);
});

test("entity comparison draws four distinct series on one axis", async ({ page }) => {
  await unlock(page);
  await page.goto("/#/usage?tab=models");
  await expect(page.getByRole("heading", { name: /模型排行/u })).toBeVisible();

  // closed by default: it costs N extra round trips
  await expect(page.locator(".chart-legend")).toHaveCount(0);
  await page.getByRole("button", { name: "展开对比" }).click();
  await expect(page).toHaveURL(/compare=1/u);

  const lines = page.locator("svg.chart-svg polyline.chart-line[data-series]");
  await expect(lines.first()).toBeVisible();
  const count = await lines.count();
  expect(count).toBeGreaterThan(1);
  expect(count).toBeLessThanOrEqual(4);

  // identity must survive without colour: each series carries its own dash
  // pattern, and no two series may be identical in both stroke and geometry
  const seen = await lines.evaluateAll((nodes) =>
    nodes.map((node) => {
      const cs = getComputedStyle(node);
      return `${cs.stroke}|${cs.strokeDasharray}|${node.getAttribute("points")?.slice(0, 60)}`;
    }),
  );
  expect(new Set(seen).size).toBe(seen.length);

  // ONE y axis: a second scale would make crossing lines meaningless
  await expect(page.locator("svg.chart-svg .chart-axis-text[data-anchor='end']").first()).toBeVisible();
});

test("monitoring exports parseable JSONL with no request bodies", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");

  const button = page.getByRole("button", { name: /导出 JSONL/u });
  await expect(button).toBeEnabled();
  // the label states the row count and whether the export is partial
  await expect(button).toContainText(/\d+ 行/u);

  const download = await Promise.all([page.waitForEvent("download"), button.click()]).then(
    ([event]) => event,
  );
  expect(download.suggestedFilename()).toMatch(/^prism-requests-[\d-]+(-partial)?\.jsonl$/u);

  const stream = await download.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString("utf8");
  const lines = text.trimEnd().split("\n");

  expect(lines.length).toBeGreaterThan(1);
  for (const line of lines) {
    expect(() => JSON.parse(line) as unknown).not.toThrow();
  }
  const header = JSON.parse(lines[0]!) as Record<string, unknown>;
  expect(header["format"]).toBe("prism.requests.v1");
  expect(header).toHaveProperty("window");
  // value-free: no body ever reaches the file, because none exists upstream
  expect(text).not.toMatch(/"(body|request_body|prompt|messages|content)"/u);
});

test("import is absent and says why, rather than being a dead control", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");
  await expect(page.locator(".export-note")).toContainText("G3");
  await expect(page.getByRole("button", { name: /导入/u })).toHaveCount(0);
});

test("trend zoom appears only above 12 buckets and never refetches", async ({ page }) => {
  await unlock(page);

  // "today" early in the day can hold 12 buckets or fewer; 24h always exceeds it
  await page.goto("/#/usage?tab=trend&range=24h");
  await expect(page.locator(".zoom-brush")).toBeVisible();

  const requests: string[] = [];
  page.on("request", (request) => {
    if (request.url().includes("/admin/analytics")) requests.push(request.url());
  });

  const end = page.locator(".zoom-handles input").nth(1);
  await end.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page).toHaveURL(/z=0-22/u);
  // the window is re-derived from buckets already fetched — narrowing must not
  // issue a new query
  expect(requests).toHaveLength(0);

  await expect(page.locator(".card-head h3").first()).toContainText("23 / 24");
  await page.getByRole("button", { name: "重置范围" }).click();
  await expect(page).not.toHaveURL(/z=/u);
});

test("clicking a bucket marks it, echoes it in the table, and is shareable", async ({
  page,
}) => {
  await unlock(page);
  await page.goto("/#/usage?tab=trend&range=24h");
  await expect(page.locator("svg.chart-svg").first()).toBeVisible();

  await page.locator("svg.chart-svg").first().click({ position: { x: 320, y: 90 } });
  await expect(page).toHaveURL(/[?&]at=\d+/u);

  // dashed rule + hollow ring: the committed selection is distinguishable from
  // the transient crosshair by shape, not colour alone
  await expect(page.locator(".chart-selected")).toHaveCount(1);
  await expect(page.locator(".chart-selected-dot")).toHaveCount(1);
  await expect(page.locator(".bucket-detail")).toBeVisible();

  // the mark is not chart-only
  await page.locator(".chart-table summary").click();
  await expect(page.locator('tr[data-selected="true"]')).toHaveCount(1);

  // Stored by start time, so the same URL reproduces the same bucket. Tested by
  // re-navigating rather than by reloading: a reload clears the in-memory
  // session by design (C6 bans browser storage) and lands on the unlock page, so
  // asserting survival across a reload would be asserting against the security
  // model rather than against the URL contract.
  const url = page.url();
  const detail = await page.locator(".bucket-detail-head strong").innerText();
  await page.goto("/#/usage?tab=overview");
  await expect(page.locator(".bucket-detail")).toHaveCount(0);
  await page.goto(url);
  await expect(page.locator(".bucket-detail-head strong")).toHaveText(detail);

  await page.getByRole("button", { name: "取消选中" }).click();
  await expect(page.locator(".chart-selected")).toHaveCount(0);
});

test("rank rows expand lazily into a per-entity panel", async ({ page }) => {
  await unlock(page);
  await page.goto("/#/usage?tab=clientKeys");
  await expect(page.getByRole("heading", { name: /Client Key 排行/u })).toBeVisible();

  // a closed table costs nothing: the panel issues its own query
  await expect(page.locator(".rank-detail-row")).toHaveCount(0);

  const toggle = page.locator(".rank-toggle").first();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");

  const panel = page.locator(".rank-detail-row");
  await expect(panel).toHaveCount(1);
  await expect(panel).toContainText("成功率");
  // spans every column, so the panel stays aligned under its row
  const columns = await page.locator("thead th").count();
  await expect(panel.locator("td")).toHaveAttribute("colspan", String(columns));

  // only one row open at a time
  await page.locator(".rank-toggle").nth(2).click();
  await expect(page.locator(".rank-detail-row")).toHaveCount(1);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
});
