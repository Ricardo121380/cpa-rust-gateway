// Batch D — the remaining contract surface.
//
// Three separate things, each wired because the list model could NOT answer the
// question on its own: a Client Key whose status can move in both directions, a
// diagnostic that spends a real upstream call, and the config plane's own
// answer about bindings.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("editing a client key needs no detail read, and says why prefix is absent", async ({
  page,
}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");

  await page.locator("tr", { hasText: "rgw_9f3c21ab04d7e6b2" }).getByRole("button", { name: "编辑" }).click();
  const sheet = page.getByRole("dialog");
  // listClientKeys returns the same ClientKey schema as getClientKey, so the
  // row IS the full record — the form pre-fills from it with no extra request.
  await expect(sheet.getByLabel("访问组")).toHaveValue("team-default");
  await expect(sheet).toContainText("整体替换");
  await expect(sheet).toContainText("prefix");
});

test("reviving a revoked key warns that the old secret works again", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");

  await page.locator("tr", { hasText: "rgw_00dead00deadbeef" }).getByRole("button", { name: "编辑" }).click();
  const sheet = page.getByRole("dialog");
  // update_client_key applies status with no transition check, and revoking
  // retains the redacted record. So this is not "re-enable an inert row".
  await expect(sheet.locator(".reveal-warning")).toHaveCount(0);
  await sheet.getByLabel("状态").selectOption("active");
  await expect(sheet.locator(".reveal-warning")).toContainText("再次可用");

  await sheet.getByRole("button", { name: "保存" }).click();
  await expect(page.locator("tr", { hasText: "rgw_00dead00deadbeef" })).toContainText("active");
});

test("channel pin says it spends a real call, and offers no free-form body", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const card = page.locator(".rt-card", { hasText: "通道诊断" });
  await expect(card).toContainText("真实请求");
  await expect(card).toContainText("消耗你自己的配额");
  // Every input is a bounded id or a closed enum. A textarea would mean the
  // panel could be used to send arbitrary content upstream.
  await expect(card.locator("textarea")).toHaveCount(0);
  await expect(card.locator('select[name="protocol"]')).toBeVisible();
  await expect(card.locator('select[name="mode"]')).toBeVisible();
});

test("a receipt keeps upstream_sent and outcome as separate facts", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const card = page.locator(".rt-card", { hasText: "通道诊断" });
  for (const [label, value] of [
    ["provider_id", "relay-a"],
    ["channel_id", "ep-relay-a-responses"],
    ["route_id", "route-1"],
    ["credential_id", "cred-relay-quota"],
    ["requested_model", "glm-5-air"],
  ] as const) {
    await card.locator(`input[name="${label}"]`).fill(value);
  }
  await card.getByRole("button", { name: "发一次真实请求" }).click();

  // failed WITHOUT reaching the provider is a local problem — different
  // information from a failure after the request was sent. Reporting only
  // "failed" throws away which half to go and look at.
  const receipt = page.locator(".rt-pin-receipt");
  await expect(receipt).toContainText("失败");
  await expect(receipt).toContainText("请求从未离开网关");
  await expect(receipt).toContainText("出口准入");
  await expect(receipt).toContainText("false");
});

test("a moved pin target is reported without claiming the config changed", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const card = page.locator(".rt-card", { hasText: "通道诊断" });
  for (const [label, value] of [
    ["provider_id", "grok-build-pool"],
    ["channel_id", "ep-grok-build"],
    ["route_id", "route-1"],
    ["credential_id", "cred-grok-old"],
    ["requested_model", "glm-5-air"],
  ] as const) {
    await card.locator(`input[name="${label}"]`).fill(value);
  }
  await card.getByRole("button", { name: "发一次真实请求" }).click();

  await expect(card.locator(".action-error")).toContainText("什么都没有发出");
  // Runtime target drift is not a configuration edit (see errors.ts).
  await expect(page.locator(".conflict-bar")).toHaveCount(0);
});

test("the config plane reports a binding the operational inventory cannot show", async ({
  page,
}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await page
    .locator("tr", { hasText: "relay-a" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  await expect(page.locator(".subresource-panel")).toBeVisible();

  await page.getByRole("button", { name: "核对绑定" }).first().click();

  const sheet = page.getByRole("dialog");
  // The panel's own table is join-driven: a binding whose credential does not
  // resolve is invisible there while still blocking validation and publish.
  await expect(sheet).toContainText("cred-deleted");
  await expect(sheet).toContainText("运营库存里没有");
  await expect(sheet.locator(".reveal-warning")).toContainText("只存在于配置里");
});
