// Settings is the only page that changes global state, so its two levers need
// to be shown reaching the shell — a language switch that misses the rail, or a
// theme choice that never writes html[data-theme], both look fine on the page
// itself and are wrong everywhere else.
import { expect, test } from "@playwright/test";
import { navigate, unlock } from "./helpers";

test("language switch reaches the whole shell, not just the settings page", async ({ page }) => {
  await unlock(page);
  await navigate(page, "设置");
  await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();

  await page.getByRole("radio", { name: "English" }).click();

  // the rail is rendered by AppShell: nav labels were computed at module scope
  // once, so a switch used to leave them in Chinese forever
  const rail = page.getByRole("navigation");
  await expect(rail.getByRole("link", { name: "Overview", exact: true })).toBeVisible();
  await expect(rail.getByRole("link", { name: "Settings", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

  // and the topbar, which is a separate glass pane. Needs a version selected —
  // the read-only note only renders for a non-draft version.
  await page.locator(".version-picker select").selectOption({ index: 1 });
  await expect(page.locator(".topbar")).toContainText("read-only");

  await page.getByRole("radio", { name: "中文" }).click();
  await expect(rail.getByRole("link", { name: "总览", exact: true })).toBeVisible();
});

test("theme choice writes the third theming layer; system writes nothing", async ({ page }) => {
  await unlock(page);
  await navigate(page, "设置");

  // "system" is the default and must leave the attribute absent, so the
  // prefers-color-scheme media query in tokens.css stays in charge
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/u);

  await page.getByRole("radio", { name: "深色" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(page.getByText("当前生效: 深色")).toBeVisible();

  await page.getByRole("radio", { name: "浅色" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

  await page.getByRole("radio", { name: "跟随系统" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/u);
});

test("settings never renders a secret, and locking clears the session", async ({ page }) => {
  await unlock(page);
  await navigate(page, "设置");

  // the full key must not be in the DOM in any form, masked or not
  const body = await page.locator("body").innerText();
  expect(body).not.toContain("a".repeat(40));
  expect(body).toContain("chars");

  await page.getByRole("button", { name: "锁定并清除密钥" }).click();
  await expect(page).toHaveURL(/#\/unlock$/u);
  await expect(page.getByRole("heading", { name: "解锁管理面板" })).toBeVisible();

  // the session is really gone, not just navigated away from
  await page.goto("/#/settings");
  await expect(page).toHaveURL(/#\/unlock$/u);
});
