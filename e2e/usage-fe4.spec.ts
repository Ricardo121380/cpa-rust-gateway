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
  // every dimension. Assert the key column, not the whole row — the row starts
  // with the rank number.
  const keyCell = page.locator(".card.tablewrap tbody tr td:nth-child(2)").first();
  await expect(keyCell).toHaveText(/^key-/u);
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
