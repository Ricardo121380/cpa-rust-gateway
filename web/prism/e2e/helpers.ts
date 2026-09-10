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

export async function selectDraft(page: Page): Promise<void> {
  await page.locator(".version-picker select").selectOption("draft-2026-08");
  await expect(page.locator(".dock")).toContainText("草稿");
}
