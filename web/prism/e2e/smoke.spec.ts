// Core flows against the fixture backend: unlock, overview observability,
// deep-link filters, version lifecycle via the draft dock.
import { expect, test } from "@playwright/test";
import { FIXTURE_PASSWORD, navigate, selectDraft, unlock } from "./helpers";

test("administrator login rejects incorrect credentials and accepts the fixture account", async ({ page }) => {
  await page.goto("/#/unlock");
  await expect(page.getByText("Management Key")).toHaveCount(0);
  await expect(page.getByText("CSRF Token")).toHaveCount(0);
  await page.getByLabel("密码", { exact: true }).fill("wrong-password");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveText("账号或密码不正确");
  await page.getByLabel("密码", { exact: true }).fill(FIXTURE_PASSWORD);
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { name: "总览" })).toBeVisible();
});

test("overview shows the real planes and deep-links into failure attribution", async ({ page }) => {
  await unlock(page);
  // The "today" KPI row and health strip were the proposed analytics shape and
  // are gone with it. What remains is the counters plane (real, from the
  // Prometheus exposition) plus the billing summary, which is one request and
  // covers the whole ledger window.
  await expect(page.getByText("网关实时计数")).toBeVisible();
  await page.getByText("事件、Token 与观测管道", { exact: true }).click();
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
  await page.getByRole("dialog", { name: "确认发布" }).getByRole("button", { name: "确认发布", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("已发布");
  await page.getByRole("button", { name: "完成" }).click();
  await expect(page.locator(".topbar")).toContainText("已发布配置");
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

test("login password supports autocomplete and accessible visibility without changing its value", async ({ page }) => {
  await page.goto("/#/unlock");
  const field = page.getByLabel("密码", { exact: true });
  await expect(field).toHaveAttribute("type", "password");
  await expect(field).toHaveAttribute("autocomplete", "current-password");
  await field.fill('  spaced-password  ');
  await page.getByRole("button", { name: "显示密码" }).click();
  await expect(field).toHaveAttribute("type", "text");
  await expect(field).toHaveValue('  spaced-password  ');
  await page.getByRole("button", { name: "隐藏密码" }).click();
  await expect(field).toHaveAttribute("type", "password");
});
