// Core flows against the fixture backend: unlock, overview observability,
// deep-link filters, version lifecycle via the draft dock.
import { expect, test } from "@playwright/test";
import { FIXTURE_CSRF, FIXTURE_KEY, navigate, selectDraft, unlock } from "./helpers";

test("unlock rejects malformed keys locally and accepts the fixture key", async ({ page }) => {
  await page.goto("/#/unlock");
  await page.getByLabel("Management Key").fill("not-a-key");
  await page.getByRole("button", { name: "解锁" }).click();
  await expect(page.getByRole("alert")).toContainText("格式不符");

  await page.getByLabel("Management Key").fill(FIXTURE_KEY);
  await page.getByRole("button", { name: "解锁" }).click();
  await expect(page.getByRole("heading", { name: "总览" })).toBeVisible();
});

test("overview shows the real planes and deep-links into failure attribution", async ({ page }) => {
  await unlock(page);
  // The "today" KPI row and health strip were the proposed analytics shape and
  // are gone with it. What remains is the counters plane (real, from the
  // Prometheus exposition) plus the billing summary, which is one request and
  // covers the whole ledger window.
  await expect(page.getByText("网关实时计数")).toBeVisible();
  await expect(page.locator(".token-mix rect").first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "计价可信度" })).toBeVisible();
  await expect(page.getByText("覆盖整个账本窗口")).toBeVisible();

  // The old link carried ?status=failed. Monitoring has no request outcome to
  // filter on and its `status` means cost confidence, so "recent failures"
  // now lands on the failure-attribution tab instead.
  await page.getByRole("link", { name: "在失败归因中查看 →" }).click();
  await expect(page).toHaveURL(/tab=failures/u);
  await expect(page.getByRole("tab", { name: "失败归因" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("draft dock publishes: anneal sheet, then version reads as active", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", { name: "发布" }).click();
  await expect(page.getByRole("dialog")).toContainText("已发布");
  await page.getByRole("button", { name: "完成" }).click();
  await expect(page.locator(".topbar")).toContainText("当前版本只读");
  await expect(page.locator(".dock")).toHaveCount(0);
});

test("versions workspace creates a draft and validates it", async ({ page }) => {
  await unlock(page);
  await navigate(page, "配置版本");
  await page.getByRole("button", { name: "创建草稿" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("版本 ID").fill("draft-e2e");
  await dialog.getByLabel(/描述/u).fill("e2e 草稿");
  await dialog.getByRole("button", { name: "创建" }).click();
  await expect(page.locator("tbody")).toContainText("draft-e2e");

  await page
    .locator("tr", { hasText: "draft-e2e" })
    .getByRole("button", { name: "验证" })
    .click();
  await expect(page.locator(".validation-card")).toContainText("route_missing_active_candidate");
});

test("unlock secrets are paste-friendly: masked text, no password type, reveal toggle", async ({
  page,
}) => {
  await page.goto("/#/unlock");
  const field = page.getByLabel("Management Key");
  // type="password" is what summons Safari's strong-password popover and
  // password-manager widgets, which cover the input and swallow paste.
  await expect(field).toHaveAttribute("type", "text");
  await expect(field).toHaveClass(/is-masked/u);
  await expect(field).toHaveAttribute("data-1p-ignore");

  const toggle = page.getByRole("button", { name: "显示密钥" }).first();
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(field).not.toHaveClass(/is-masked/u);
});

test("pasted secrets survive newlines, quotes and assignment prefixes", async ({ page }) => {
  await page.goto("/#/unlock");
  await page.getByLabel("Management Key").fill(`MGMT_KEY="${FIXTURE_KEY}"\n`);
  await page.getByLabel(/CSRF Token/u).fill(`  ${FIXTURE_CSRF}\n`);
  await page.getByRole("button", { name: "解锁" }).click();
  await expect(page.getByRole("heading", { name: "总览" })).toBeVisible();
});
