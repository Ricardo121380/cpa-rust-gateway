import { expect, test } from "@playwright/test";
import { unlock } from "./helpers";

// The live-counters plane on the overview: real contract op
// getObservabilityMetrics, served as Prometheus text. These assertions are
// about what the plane may claim, not just that it renders — the counters are
// cumulative with no time window, so the page must not read as analytics.

test("overview lights the live counters from the Prometheus exposition", async ({ page }) => {
  await unlock(page);

  const section = page.locator(".stat-row").first();
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
  const card = page.locator(".card").filter({ hasText: "观测管道健康" });
  await expect(card.locator(".badge-good")).toHaveText("必需事件无丢失");
  await expect(card.locator(".badge-critical")).toHaveCount(0);
  // The fixture does shed diagnostics — that must read as design, not failure.
  await expect(card).toContainText("背压设计,非故障");
});

test("the two observability planes coexist without duplicating a card", async ({ page }) => {
  await unlock(page);
  // Fixture dev has G3 analytics available, so the page shows both planes.
  await expect(page.getByText("网关实时计数")).toBeVisible();
  await expect(page.getByRole("heading", { name: "流量趋势(今日,按小时)" })).toBeVisible();

  // Exactly one token-composition card: the cumulative one stands down when
  // the time-dimension one is live.
  await expect(page.getByRole("heading", { name: /^Token 构成/u })).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "Token 构成(今日)" })).toBeVisible();

  // Pipeline health has no G3 equivalent and is always present.
  await expect(page.getByRole("heading", { name: /观测管道健康/u })).toBeVisible();
});

test("counters accumulate a visit delta across scrapes", async ({ page }) => {
  // The app sets refetchOnWindowFocus:false globally, and the fixture backend
  // is an options.fetch seam that page.route cannot see — so the only way to
  // reach a second scrape is the 15s poll. Drive it with the fake clock
  // instead of spending 15s of wall time.
  await page.clock.install();
  await unlock(page);
  await expect(page.getByText("网关实时计数")).toBeVisible();

  // Before a second scrape there is no observation window, so no delta is
  // claimed — "+0" here would describe a window that never happened.
  const attemptsSub = page.locator(".stat-tile").first().locator(".stat-sub");
  await expect(attemptsSub).toContainText("失败");
  await expect(attemptsSub).not.toContainText("本页 +");

  await page.clock.runFor(16_000);
  await expect(attemptsSub).toContainText("本页 +");
});
