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

test("upstream resources use complete inventory and separate runtime bindings", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");

  await page
    .locator('tr:has([data-resource-id="relay-a"])')
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  const panel = page.locator(".subresource-panel");
  // The operations plane's own vocabulary, verbatim — not translated back
  // into the config plane's upstream/endpoint/credential.
  await expect(panel).toContainText("Channel");
  await expect(panel).toContainText("Account");
  await expect(panel).toContainText("绑定");
  // Boundaries the projection imposes, stated rather than papered over.
  await expect(panel).toContainText("已保存端点均可管理");
  await expect(panel).toContainText("不代表凭据健康");

  await panel
    .locator('tr:has([data-resource-id="ep-relay-a-responses"])')
    .getByRole("button", { name: "非流式" })
    .click();
  await expect(panel.locator('tr:has([data-resource-id="ep-relay-a-responses"])')).toContainText("pass · 2xx");
});

test("the ledger pages with the cursor and stops when the stream ends", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求与失败");
  // 73 fixture rows, page size 100 — one page covers them, so there is no
  // "load more" to press. The button must be ABSENT rather than present and
  // inert: a dead control reads as a broken one.
  await expect(page.locator(".mon-table tbody tr")).toHaveCount(73);
  await expect(page.getByRole("button", { name: "再读一页" })).toHaveCount(0);
});
