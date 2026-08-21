// 390px — an iPhone-class width, and the narrowest the CSS claims to support.
//
// Until now nothing ran below the desktop viewport, so every @media rule in the
// app was shipped unverified. This project asks one question of every page:
// does the content fit, or does the body scroll sideways? A page that scrolls
// horizontally on a phone is a page where the right-hand column — which is
// where every action button lives — is simply unreachable.
import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

const PAGES = [
  "总览",
  "用量分析",
  "请求监控",
  "计费与价格",
  "配置版本",
  "上游",
  "模型与路由",
  "访问控制",
  "出口策略",
  "运行时",
  "审计与备份",
  "设置",
];

/** Content that is clipped and unreachable, not merely ugly.
 *
 * The document can never scroll sideways here: the main scroller sets
 * `overflow-x: hidden` (app.css). So a naive scrollWidth check can never fail —
 * anything too wide is silently CUT OFF instead, with no scrollbar to reveal it
 * and no way for a person to reach it. The real question is therefore: does any
 * element extend past its scroller's right edge without a horizontally
 * scrollable ancestor to reach it? Wide tables inside `overflow-x: auto` are
 * fine, and that is exactly the distinction this makes.
 */
async function clippedElements(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const scroller = document.querySelector(".canvas, main, [class*='scroll']") ?? document.body;
    const limit = scroller.getBoundingClientRect().right;
    const out: string[] = [];
    for (const element of Array.from(document.querySelectorAll("body *"))) {
      const box = element.getBoundingClientRect();
      if (box.width === 0 || box.height === 0) continue;
      if (box.right <= limit + 1) continue;
      let reachable = false;
      let parent: Element | null = element.parentElement;
      while (parent !== null && parent !== document.body) {
        const overflowX = getComputedStyle(parent).overflowX;
        if (overflowX === "auto" || overflowX === "scroll") {
          reachable = true;
          break;
        }
        parent = parent.parentElement;
      }
      if (!reachable) {
        const label = `${element.tagName.toLowerCase()}.${(element.className || "")
          .toString()
          .split(" ")
          .filter(Boolean)
          .slice(0, 2)
          .join(".")}`;
        out.push(`${label} +${Math.round(box.right - limit)}px`);
      }
    }
    return [...new Set(out)].slice(0, 8);
  });
}

test("no page clips content out of reach at 390px", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);

  const offenders: string[] = [];
  for (const label of PAGES) {
    await navigate(page, label);
    await page.waitForTimeout(200);
    const clipped = await clippedElements(page);
    if (clipped.length > 0) {
      offenders.push(`${label} → ${clipped.join(", ")}`);
    }
  }
  expect(offenders.join("\n")).toBe("");
});

test("the rail stays reachable and the version picker stays usable", async ({ page }) => {
  await unlock(page);

  // The rail is the only way between pages; if it collapses off-screen at this
  // width the app is unusable rather than merely ugly.
  const rail = page.getByRole("navigation");
  await expect(rail).toBeVisible();
  const box = await rail.boundingBox();
  expect(box).not.toBeNull();
  expect(box?.x ?? -1).toBeGreaterThanOrEqual(0);

  await expect(page.locator(".version-picker select")).toBeVisible();
});
