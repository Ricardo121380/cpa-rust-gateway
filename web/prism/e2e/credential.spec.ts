import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// Runtime projections must retain navigation to the shared account inspector.
// Internal references are selectors only; they must not become visible labels.

async function openRuntime(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行诊断");
}

test("an account in the availability matrix opens its detail", async ({ page }) => {
  await openRuntime(page);

  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-codex-oauth"])').click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toHaveAccessibleName("账号详情");
  await expect(sheet.getByRole("heading", { name: "ops@fixture.example" })).toBeVisible();
  await sheet.getByRole("button", { name: "配置", exact: true }).click();
  await expect(sheet).toContainText("Codex / ChatGPT 授权");
});

test("G5 metadata renders, and its all-null case says so instead of showing blanks", async ({
  page,
}) => {
  await openRuntime(page);

  // The oauth credential carries a full identity.
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-codex-oauth"])').click();
  const rich = page.getByRole("dialog");
  await expect(rich).toContainText("ops@fixture.example");
  await expect(rich).toContainText("Plus");
  await rich.getByRole("button", { name: "配置", exact: true }).click();
  await expect(rich).toContainText("direct_oauth");
  await rich.getByRole("button", { name: "关闭", exact: true }).click();

  // The api_key one carries nothing — every metadata field is nullable.
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-relay-key"])').click();
  const sparse = page.getByRole("dialog");
  await sparse.getByRole("button", { name: "配置", exact: true }).click();
  await expect(sparse).toContainText("尚无账号、套餐或额度观测");
});

test("token rotation advances the credential revision", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-codex-oauth"])').click();
  const sheet = page.getByRole("dialog");

  await sheet.getByRole("button", { name: "配置", exact: true }).click();
  const revision = () => page.evaluate(async () => {
    const { call } = await import("/src/api/client.ts");
    return (await call<{ revision: number }>("getCredential", { path: { credential_id: "cred-codex-oauth" } }, { versionScoped: true })).revision;
  });
  const before = await revision();
  await sheet.getByRole("button", { name: "轮换令牌" }).click();
  await expect(sheet).toContainText("令牌已轮换");
  await expect.poll(revision).toBe(before + 1);
});

test("rotation is offered only where a token exists", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-relay-key"])').click();
  const sheet = page.getByRole("dialog");
  await sheet.getByRole("button", { name: "配置", exact: true }).click();
  await expect(sheet).toContainText("API Key / Token");
  await expect(sheet.getByRole("button", { name: "轮换令牌" })).toHaveCount(0);
  await expect(sheet.getByRole("button", { name: "重新授权" })).toHaveCount(0);
});

test("re-authorisation reaches the wizard and comes back to the credential", async ({ page }) => {
  await openRuntime(page);
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-codex-oauth"])').click();
  await page.getByRole("dialog").getByRole("button", { name: "配置", exact: true }).click();
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
  await page.locator(".rt-matrix thead").locator('button:has([data-resource-id="cred-codex-oauth"])').click();
  const sheet = page.getByRole("dialog");
  await sheet.getByRole("button", { name: "配置", exact: true }).click();
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
    .locator(".provider-card", { hasText: "Kiro" })
    .first()
    .getByRole("button", { name: "接口与账号" })
    .click();
  await expect(page.getByRole("button", { name: "添加账号", exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "新建接口", exact: true })).toBeEnabled();
});

test("the account row opens the credential sheet from the pool inventory", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await page
    .locator(".provider-card", { hasText: "中转站 A" })
    .first()
    .getByRole("button", { name: "接口与账号" })
    .click();

  const panel = page.locator(".subresource-panel");
  // The operations status vocabulary reaches the screen unmapped.
  // anchored: the id also appears in the binding table below
  await expect(panel.locator('article[data-resource-id="cred-codex-oauth"]')).toContainText("已启用");

  await panel.locator('article[data-resource-id="cred-codex-oauth"]').getByRole("button", { name: "详情" }).click();
  await expect(page.getByRole("dialog")).toHaveAccessibleName("账号详情");
  await expect(page.getByRole("dialog").getByRole("heading", { name: "alex@example.test" })).toBeVisible();
});
