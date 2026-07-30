// Guards for the material itself. These assert the two mechanisms that were
// silently inert once before: a lens filter with no displacement map is a clear
// window, not glass, and a config-version state that never reaches the lens
// leaves the whole material-semantics vocabulary decorative. Both failed
// invisibly — the pane still rendered — so only a measurement catches them.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

/** Reads the live primitive values out of one pane's <filter>. */
async function lens(page: import("@playwright/test").Page, pane: string) {
  return page.evaluate((id) => {
    const f = document.getElementById(`prism-lens-${id}`);
    if (f === null) return null;
    const image = f.querySelector("feImage");
    return {
      href: image?.getAttribute("href") ?? "",
      width: Number(image?.getAttribute("width") ?? 0),
      frost: Number(f.querySelectorAll("feGaussianBlur")[0]?.getAttribute("stdDeviation") ?? 0),
      sat: Number(f.querySelector('feColorMatrix[type="saturate"]')?.getAttribute("values") ?? 0),
    };
  }, pane);
}

test("every mounted chrome pane gets a real displacement map", async ({ page }) => {
  await unlock(page);
  await expect(page.locator("html")).toHaveAttribute("data-lens", "on");

  for (const pane of ["topbar", "rail"]) {
    const l = await lens(page, pane);
    expect(l?.href, `${pane} map`).toContain("data:image/png");
    expect(l?.width, `${pane} map width`).toBeGreaterThan(100);
  }

  // The dock mounts later, when a draft is selected. It is position:fixed, so it
  // never resizes the body — the original ResizeObserver-only wiring left its
  // feImage at href="" and the pane leaked every glyph underneath.
  await selectDraft(page);
  const dock = await lens(page, "dock");
  expect(dock?.href).toContain("data:image/png");
  expect(dock?.width).toBeGreaterThan(100);
});

test("config-version state drives the lens, and publishing anneals it", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  // Selecting the draft starts its own anneal; sample the endpoint only once it
  // has settled, or the "draft" reading is itself a mid-flight value.
  await page.waitForTimeout(900);
  const draft = await lens(page, "rail");

  await navigate(page, "配置版本");
  await page.locator(".dock").getByRole("button", { name: "发布" }).click();
  await page.getByRole("button", { name: "完成" }).click();
  await expect(page.locator(".topbar")).toContainText("当前版本只读");

  // Sampled inside the 600ms anneal: SVG filter primitives are attributes, not
  // animatable CSS properties, so `transition: backdrop-filter` cannot move
  // them — without the JS tween this snapped in one frame.
  await page.waitForTimeout(120);
  const mid = await lens(page, "rail");
  await page.waitForTimeout(900);
  const active = await lens(page, "rail");

  // draft = frosted and desaturated; active = clear and vivid
  expect(draft?.frost).toBeGreaterThan(active?.frost ?? 0);
  expect(draft?.sat).toBeLessThan(active?.sat ?? 0);
  // mid-flight value sits strictly between the endpoints
  expect(mid?.frost).toBeLessThan(draft?.frost ?? 0);
  expect(mid?.frost).toBeGreaterThan(active?.frost ?? 0);
});

test("content surfaces share one left edge across pages", async ({ page }) => {
  await unlock(page);
  const lefts: Record<string, number[]> = {};
  for (const [label, name] of [
    ["请求监控", "monitoring"],
    ["用量分析", "usage"],
  ] as const) {
    await navigate(page, label);
    await expect(page.getByRole("heading", { name: label })).toBeVisible();
    lefts[name] = await page.evaluate(() => {
      const canvas = document.querySelector(".canvas");
      if (canvas === null) return [];
      const full = canvas.getBoundingClientRect().width;
      return (
        [...canvas.querySelectorAll(".card")]
          // full-bleed surfaces only: a grid's second column legitimately starts
          // elsewhere, so measuring it would compare unrelated edges
          .filter((el) => el.getBoundingClientRect().width > full * 0.8)
          .map((el) => Math.round(el.getBoundingClientRect().left))
      );
    });
  }

  // Usage wraps its body in a [role=tabpanel]; monitoring does not. That extra
  // level used to strand Usage's cards 24px right of everything else, because
  // the rail-underlap shift only matched two levels of nesting.
  const all = [...(lefts["monitoring"] ?? []), ...(lefts["usage"] ?? [])];
  expect(all.length).toBeGreaterThan(2);
  expect(new Set(all).size, `left edges: ${JSON.stringify(lefts)}`).toBe(1);
});
