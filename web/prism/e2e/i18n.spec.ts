// The language switch, and what it actually reaches.
//
// Batch D6 wired the skeleton layer only: chrome plus every state vocabulary.
// These tests pin BOTH halves of that — the part that translates, and the part
// that openly does not — because a switch that silently under-delivers is the
// defect, not the untranslated prose itself.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function switchToEnglish(page: import("@playwright/test").Page): Promise<void> {
  await navigate(page, "设置");
  await page.getByRole("radio", { name: "English" }).click();
}

test("state vocabularies translate, glyph and enum value do not change", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await switchToEnglish(page);
  await page.getByRole("navigation").getByRole("link", { name: "Runtime", exact: true }).click();

  // The pool card's two axes are the densest chip surface in the app.
  const row = page.locator("tr", { hasText: "cred-grok-oauth" });
  await expect(row.locator('.rt-chip[data-state="reauth_required"]')).toContainText("Reauth needed");
  await expect(row.locator('.rt-chip[data-state="unauthorized"]')).toContainText("Refused");
  // The raw contract value stays available to assistive tech in both languages —
  // it is an identifier, not copy.
  await expect(row.locator('.rt-chip[data-state="unauthorized"]')).toContainText("unauthorized");
});

test("the same word in two vocabularies gets two translations", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await switchToEnglish(page);
  await page.getByRole("navigation").getByRole("link", { name: "Runtime", exact: true }).click();

  // `disabled` is an operator-disabled credential on the auth axis and an
  // administratively disabled egress in the egress domain. A flat i18n key
  // space would have merged them; the vocabularies keep them apart.
  const egress = page.locator('.rt-domain[data-domain="egress"]');
  await expect(egress.locator('.rt-chip[data-state="disabled"]').first()).toContainText("Disabled");
  await expect(egress.locator('.rt-chip[data-state="probe_due"]')).toContainText("Probe allowed");
  // Permission to probe must not read as recovered, in either language.
  await expect(egress.locator('.rt-chip[data-state="probe_due"]')).not.toContainText("Available");
});

test("the switch states what English does not cover", async ({ page }) => {
  await unlock(page);
  await switchToEnglish(page);

  // The old copy promised "UI copy switches immediately", which was true of the
  // chrome and false of every page body.
  await expect(page.locator(".settings-help").filter({ hasText: "English currently covers" })).toContainText(
    "explanatory prose on each page is still Chinese",
  );
  await expect(page.getByText("UI copy switches immediately")).toHaveCount(0);
});
