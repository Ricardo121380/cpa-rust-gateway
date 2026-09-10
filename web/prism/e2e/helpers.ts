import { expect, type Page } from "@playwright/test";

export const FIXTURE_PASSWORD = "Prism-demo-2026";
export const FIXTURE_KEY = `mgmt_${"a".repeat(40)}`;
export const FIXTURE_CSRF = `csrf_${"b".repeat(40)}`;

export async function unlock(page: Page): Promise<void> {
  await page.goto("/#/unlock");
  await page.getByLabel("账号", { exact: true }).fill("admin");
  await page.getByLabel("密码", { exact: true }).fill(FIXTURE_PASSWORD);
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { name: "总览" })).toBeVisible();
}

/** Navigate via the rail (scoped: page bodies also link to the same routes). */
export async function navigate(page: Page, label: string): Promise<void> {
  if (!(await page.getByRole("navigation").isVisible())) {
    await page.locator("#nav-toggle").click();
  }
  await page.getByRole("navigation").getByRole("link", { name: label, exact: true }).click();
}

export async function selectVersion(page: Page, id: string): Promise<void> {
  const previousHash = new URL(page.url()).hash;
  await navigate(page, "配置版本");
  const row = page.locator(`[data-version-id="${id}"]`);
  const button = row.getByRole("button", { name: /^(正在查看|编辑草稿|查看历史|查看已发布配置)$/u });
  await expect(button).toBeVisible();
  if (await button.isEnabled()) await button.click();
  await expect(page.locator(".configuration-context")).toHaveAttribute("data-context-version", id);
  if (previousHash !== "#/versions") await page.goto(`/${previousHash}`);
}

export async function selectDraft(page: Page): Promise<void> {
  await selectVersion(page, "draft-2026-08");
  await expect(page.locator(".dock")).toContainText("草稿");
}

/** Simulate a gateway with drafts but no published configuration. */
export async function clearVersionForTest(page: Page): Promise<void> {
  await expect(page.locator(".configuration-context")).toHaveAttribute("data-context-version", "v-2026-07");
  await page.evaluate(async () => {
    const clientPath = "/src/generated/management-client.ts";
    const { ManagementApi } = await import(clientPath);
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async function(this: unknown, operation: string, request: unknown) {
      const response = await original.call(this, operation, request);
      if (operation !== "listConfigVersions") return response;
      const versions = await response.json();
      return new Response(JSON.stringify(versions.filter((row: { status: string }) => row.status !== "active")), { status: response.status, headers: response.headers });
    };
    const modulePath = "/src/features/config-versions/versionStore.ts";
    const { useVersionStore } = await import(modulePath);
    useVersionStore.getState().reset();
  });
  await expect(page.locator(".configuration-context")).not.toHaveAttribute("data-context-version");
}
