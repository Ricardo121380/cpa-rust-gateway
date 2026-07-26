// Safety-critical and diagnostic flows: reveal-once client key issuance,
// upstream subresources with endpoint test, OAuth device-flow polling,
// monitoring cursor paging.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("client key issuance is reveal-once and revoke is two-step", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
  await page.getByRole("button", { name: "签发 Client Key" }).click();

  const issueDialog = page.getByRole("dialog");
  await issueDialog.getByLabel("Key ID").fill("key-e2e");
  await issueDialog.getByRole("button", { name: "签发" }).click();

  const reveal = page.getByRole("dialog");
  await expect(reveal).toContainText("只显示这一次");
  const key = await reveal.locator(".reveal-key").textContent();
  expect(key).toMatch(/^rgw_[0-9a-f]{16}_[0-9a-f]{64}$/u);
  await reveal.getByRole("button", { name: "我已保存,关闭" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);

  const prefix = (key as string).slice(0, 20);
  await expect(page.locator("tbody").last()).toContainText(prefix);

  // The list shows only the prefix — there is no id/name column (this is the
  // G5 metadata gap in practice: a freshly issued key can only be located by
  // the prefix captured from the reveal sheet).
  const row = page.locator("tr", { hasText: prefix }).first();
  await row.getByRole("button", { name: "吊销" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("不可逆");
  await confirm.getByRole("button", { name: "确认吊销" }).click();
  await expect(page.locator("tr", { hasText: prefix })).toContainText("revoked");
});

test("upstream subresources: three tables, endpoint test badge", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");

  await page
    .locator("tr", { hasText: "relay-a" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  const panel = page.locator(".subresource-panel");
  await expect(panel).toContainText("端点");
  await expect(panel).toContainText("凭据");
  await expect(panel).toContainText("绑定");

  await panel
    .getByRole("row", { name: /ep-relay-a-responses openai/u })
    .getByRole("button", { name: "非流式" })
    .click();
  await expect(panel.getByRole("row", { name: /ep-relay-a-responses openai/u })).toContainText("pass · 2xx");
});

test("oauth wizard polls to complete", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");

  await page
    .locator("tr", { hasText: "grok-build-pool" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  await page.getByRole("button", { name: "OAuth 授权" }).click();
  const wizard = page.getByRole("dialog");
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await expect(wizard).toContainText("pending");
  await expect(wizard).toContainText("complete", { timeout: 12_000 });
  await wizard.getByRole("button", { name: "完成" }).click();
});

test("monitoring paginates events with the cursor", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");
  await expect(page.locator("tbody tr")).toHaveCount(25);
  await page.getByRole("button", { name: "加载更多" }).click();
  await expect(page.locator("tbody tr")).toHaveCount(50);
});
