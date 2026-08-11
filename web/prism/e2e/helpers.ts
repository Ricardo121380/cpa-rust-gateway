import { expect, type Page } from "@playwright/test";

export const FIXTURE_KEY = `mgmt_${"a".repeat(40)}`;
export const FIXTURE_CSRF = `csrf_${"b".repeat(40)}`;

export async function unlock(page: Page): Promise<void> {
  await page.goto("/#/unlock");
  await page.getByLabel("Management Key").fill(FIXTURE_KEY);
  await page.getByLabel(/CSRF Token/u).fill(FIXTURE_CSRF);
  await page.getByRole("button", { name: "解锁" }).click();
  await expect(page.getByRole("heading", { name: "总览" })).toBeVisible();
}

/** Navigate via the rail (scoped: page bodies also link to the same routes). */
export async function navigate(page: Page, label: string): Promise<void> {
  await page.getByRole("navigation").getByRole("link", { name: label, exact: true }).click();
}

export async function selectDraft(page: Page): Promise<void> {
  await page.locator(".version-picker select").selectOption("draft-2026-08");
  await expect(page.locator(".dock")).toContainText("草稿");
}
