import { expect, test } from "@playwright/test";
import { unlock } from "./helpers";

// The live-counters plane on the overview: real contract op
// getObservabilityMetrics, served as Prometheus text. These assertions are
// about what the plane may claim, not just that it renders — the counters are
// cumulative with no time window, so the page must not read as analytics.

test("overview lights the live counters from the Prometheus exposition", async ({ page }) => {
  await unlock(page);
  await page.getByText("进程计数与运行事件", { exact: true }).click();

  const section = page.locator("details.overview-telemetry").filter({ has: page.getByText("进程计数与运行事件", { exact: true }) }).locator(".stat-row");
  await expect(page.getByText("网关实时计数")).toBeVisible();
  await expect(page.getByText("自进程启动累计")).toBeVisible();

  // Values come from the exposition, not from a placeholder.
  await expect(section.getByText("上游尝试")).toBeVisible();
  const attempts = section.locator(".stat-tile").first().locator(".stat-value");
  await expect(attempts).not.toHaveText("—");
  await expect(attempts).not.toHaveText("0");

  // Success rate is a real ratio, not a hardcoded 100%.
  const rate = section.locator(".stat-tile").nth(1).locator(".stat-value");
  await expect(rate).toHaveText(/^\d+\.\d{2}%$/u);
  const parsed = Number((await rate.innerText()).replace("%", ""));
  expect(parsed).toBeGreaterThan(0);
  expect(parsed).toBeLessThan(100);
});

// Note: the cumulative Token bar only renders when G3 analytics is absent,
// which fixture dev never is — so it has no E2E path. Its shaping is covered
// by src/features/overview/metrics.test.ts and it reuses the already-tested
// TokenMixBar unchanged.

test("pipeline health reports a clean Required path without crying wolf on shed diagnostics", async ({
  page,
}) => {
  await unlock(page);
  await page.getByText("进程计数与运行事件", { exact: true }).click();
  await page.getByText("事件、Token 与观测管道", { exact: true }).click();
  const card = page.locator(".card").filter({ hasText: "观测管道健康" });
  await expect(card.locator(".badge-good")).toHaveText("必需事件无丢失");
  await expect(card.locator(".badge-critical")).toHaveCount(0);
  // The fixture does shed diagnostics — that must read as design, not failure.
  await expect(card).toContainText("背压设计,非故障");
});

test("the counters plane stands alone, and says what it cannot show", async ({ page }) => {
  await unlock(page);
  await page.getByText("进程计数与运行事件", { exact: true }).click();
  await expect(page.getByText("网关实时计数")).toBeVisible();

  await page.getByText("事件、Token 与观测管道", { exact: true }).click();

  // Cumulative counters remain distinct from the request trend above them.
  // Their expanded section must not imply its own time-window analytics.
  await expect(page.getByRole("heading", { name: /^Token 构成/u })).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "Token 构成(累计)" })).toBeVisible();
  await expect(page.locator(".overview-telemetry svg.chart-svg")).toHaveCount(0);
  await expect(page.locator(".health-strip")).toHaveCount(0);

  // Pipeline health reads the exposition directly and is always present.
  await expect(page.getByRole("heading", { name: /观测管道健康/u })).toBeVisible();

  // The absence is explained, not silent — and it points at the pages that can
  // answer the question properly.
  await page.getByText("分析范围", { exact: true }).click();
  await expect(page.getByText(/计数器为进程累计值；请求日志提供独立的时间桶/u)).toBeVisible();
  await expect(page.getByRole("link", { name: "前往用量分析 →" })).toBeVisible();
});

test("counters accumulate a visit delta across scrapes", async ({ page }) => {
  // The app sets refetchOnWindowFocus:false globally, and the fixture backend
  // is an options.fetch seam that page.route cannot see — so the only way to
  // reach a second scrape is the 15s poll. Drive it with the fake clock
  // instead of spending 15s of wall time.
  await page.clock.install();
  await unlock(page);
  await page.getByText("进程计数与运行事件", { exact: true }).click();
  await expect(page.getByText("网关实时计数")).toBeVisible();

  // Before a second scrape there is no observation window, so no delta is
  // claimed — "+0" here would describe a window that never happened.
  const attemptsSub = page.locator(".overview-telemetry .stat-tile").first().locator(".stat-sub");
  await expect(attemptsSub).toContainText("失败");
  await expect(attemptsSub).not.toContainText("本页 +");

  await page.clock.runFor(16_000);
  await expect(attemptsSub).toContainText("本页 +");
});
