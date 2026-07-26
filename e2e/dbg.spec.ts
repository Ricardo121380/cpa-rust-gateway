import { test } from "@playwright/test";
import { unlock } from "./helpers";
test("dbg css", async ({ page }) => {
  await unlock(page);
  await page.waitForTimeout(1200);
  const info = await page.evaluate(() => {
    const strip = document.querySelector(".health-strip") as HTMLElement;
    const cell = document.querySelector(".health-cell") as HTMLElement;
    return {
      stripDisplay: getComputedStyle(strip).display,
      cellWidth: getComputedStyle(cell).width,
      cellBg: getComputedStyle(cell).backgroundColor,
      sheets: [...document.styleSheets].map((s) => {
        try { return (s.cssRules?.length ?? 0); } catch { return -1; }
      }),
      hasRule: [...document.styleSheets].some((s) => {
        try { return [...(s.cssRules ?? [])].some((r) => r.cssText.includes(".health-strip")); } catch { return false; }
      }),
    };
  });
  console.log("CSS:", JSON.stringify(info));
});
