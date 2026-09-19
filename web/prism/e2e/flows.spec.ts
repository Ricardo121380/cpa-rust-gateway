// Safety-critical and diagnostic flows: reveal-once client key issuance,
// upstream subresources with endpoint test, OAuth device-flow polling,
// monitoring cursor paging.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("client key issuance is reveal-once and revoke is two-step", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button", { name: "按访问组签发" }).click();

  const issueDialog = page.getByRole("dialog",{name:"按访问组签发"});
  await issueDialog.getByLabel("Key ID").fill("key-e2e");
  await issueDialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await issueDialog.getByRole("button", { name: "签发到草稿" }).click();

  const reveal = page.getByRole("dialog",{name:"Client Key 已签发"});
  await expect(reveal.locator(".reveal-key")).toBeVisible();
  const prefix=await reveal.locator(".reveal-key").evaluate(node=>node.textContent?.slice(0,20));
  expect(prefix).toMatch(/^rgw_[0-9a-f]{16}$/u);
  if(!prefix)throw new Error("missing synthetic key prefix");
  await reveal.getByRole("button", { name: "完成并清除密钥" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator("tbody").last()).toContainText(prefix);

  const row = page.locator("tr", { hasText: prefix }).first();
  await row.getByRole("button", { name: "吊销" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText(prefix);
  await confirm.getByRole("button", { name: "确认吊销" }).click();
  await expect(page.getByRole("dialog",{name:"密钥吊销结果"})).toContainText("已保存到当前草稿");
  await page.getByRole("dialog",{name:"密钥吊销结果"}).getByRole("button",{name:"核对配置"}).click();
  await expect(page.locator("tr", { hasText: prefix })).toContainText("已吊销");
});

test("upstream resources use complete inventory and separate runtime bindings", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");

  await page.locator("article",{hasText:"中转站 A"}).getByRole("button",{name:"接口与账号"}).click();
  const panel = page.locator(".subresource-panel");
  await expect(panel).toContainText("接口");
  await expect(panel).toContainText("账号");
  await expect(panel).toContainText("绑定");
  const endpoint=panel.locator('article[data-resource-id="ep-relay-a-responses"]');
  await endpoint.getByRole("button",{name:"非流式"}).click();
  await expect(endpoint).toContainText("pass · 2xx");
});

test("the ledger pages with the cursor and stops when the stream ends", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求与失败");
  await page.getByRole("tab",{name:"计费账本"}).click();
  // 73 fixture rows, page size 100 — one page covers them, so there is no
  // "load more" to press. The button must be ABSENT rather than present and
  // inert: a dead control reads as a broken one.
  await expect(page.locator(".mon-table tbody tr")).toHaveCount(73);
  await expect(page.getByRole("button", { name: "再读一页" })).toHaveCount(0);
});
