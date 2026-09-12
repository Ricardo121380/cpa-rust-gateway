import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// The credential surface. Until G1 there is no listCredentials, so the runtime
// projections are the only production enumeration — which makes the runtime
// page the entry point, and makes "can you get there at all" part of the test.

async function openRuntime(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行诊断");
}

test("a credential id in the availability matrix opens its detail", async ({ page }) => {
  await openRuntime(page);

  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-codex-oauth" }).click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toHaveAccessibleName("账号详情");
  await expect(sheet.getByRole("heading", { name: "ops@fixture.example" })).toBeVisible();
  await expect(sheet).toContainText("relay-a");
  await expect(sheet).toContainText("oauth");
});

test("G5 metadata renders, and its all-null case says so instead of showing blanks", async ({
  page,
}) => {
  await openRuntime(page);

  // The oauth credential carries a full identity.
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-codex-oauth" }).click();
  const rich = page.getByRole("dialog");
  await expect(rich).toContainText("ops@fixture.example");
  await expect(rich).toContainText("Plus");
  await expect(rich).toContainText("direct_oauth");
  await rich.getByRole("button", { name: "关闭", exact: true }).click();

  // The api_key one carries nothing — every metadata field is nullable.
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-relay-key" }).click();
  const sparse = page.getByRole("dialog");
  await expect(sparse).toContainText("没有记录平台、账号、套餐或配额");
});

test("token rotation advances the credential revision", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-codex-oauth" }).click();
  const sheet = page.getByRole("dialog");

  const before = await sheet.locator("tbody tr", { hasText: "修订" }).locator("td.mono").innerText();
  await sheet.getByRole("button", { name: "轮换令牌" }).click();
  await expect(sheet).toContainText("令牌已轮换");
  const after = await sheet.locator("tbody tr", { hasText: "修订" }).locator("td.mono").innerText();
  expect(Number(after)).toBe(Number(before) + 1);
});

test("rotation is offered only where a token exists", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-relay-key" }).click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("bearer");
  await expect(sheet.getByRole("button", { name: "轮换令牌" })).toHaveCount(0);
  await expect(sheet.getByRole("button", { name: "重新授权" })).toHaveCount(0);
});

test("re-authorisation reaches the wizard and comes back to the credential", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-codex-oauth" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "重新授权" }).click();

  const wizard = page.getByRole("dialog");
  await expect(wizard).toHaveAccessibleName("重新授权");
  await expect(wizard).toContainText("ops@fixture.example");
  await expect(wizard).not.toContainText("cred-codex-oauth");
  await wizard.getByRole("button", { name: "关闭", exact: true }).click();
  // closing the wizard returns to the credential, not to the page
  await expect(page.getByRole("dialog")).toHaveAccessibleName("账号详情");
});

test("no secret material reaches the DOM", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").getByRole("button", { name: "cred-codex-oauth" }).click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("秘密");
  // presence is reported; the value never is
  await expect(sheet).toContainText("已配置");
  const text = await sheet.innerText();
  expect(text).not.toContain("secret");
  expect(text).not.toMatch(/[A-Za-z0-9_-]{32,}/u);
});

test("an empty provider still exposes account and endpoint creation", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  // kiro-sub has an upstream row but no endpoint-credential binding, so the
  // projection returns zero rows for it. That is not "no upstream".
  await page
    .locator("tr", { hasText: "kiro-sub" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  await expect(page.getByRole("button", { name: "新建 Account", exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "新建 Channel", exact: true })).toBeEnabled();
});

test("the account row opens the credential sheet from the pool inventory", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await page
    .locator("tr", { hasText: "grok-build-pool" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();

  const panel = page.locator(".subresource-panel");
  // The operations status vocabulary reaches the screen unmapped.
  // anchored: the id also appears in the binding table below
  await expect(panel.getByRole("row", { name: /^cred-grok-oauth bearer/u })).toContainText("active");

  await panel.getByRole("row", { name: /cred-grok-oauth/u }).getByRole("button", { name: "详情" }).click();
  await expect(page.getByRole("dialog")).toHaveAccessibleName("账号详情");
  await expect(page.getByRole("dialog").getByRole("heading", { name: "未提供账号身份" })).toBeVisible();
});
