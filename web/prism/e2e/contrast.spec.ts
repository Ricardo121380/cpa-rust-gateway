import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// Anchors wear --tint app-wide (there was no anchor rule at all before, so
// every link rendered in the UA's #0000EE). Measured: --tint is 4.70:1 on
// --surface light / 4.94:1 dark, but only 4.39:1 on --surface-2 in light —
// under AA. That surface is one card nesting away, so this walks every page in
// both themes and measures each link against whatever is actually behind it.
//
// Container links (.count-tile) are skipped on purpose: their content is child
// elements carrying their own ink, so the anchor's colour paints nothing.

const PAGES = [
  "总览",
  "用量分析",
  "请求监控",
  "配置版本",
  "上游",
  "模型与路由",
  "访问控制",
  "出口策略",
  "运行时",
  "审计与备份",
  "设置",
];

function findFailures(): string[] {
  const lin = (c: number): number => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  const lum = (rgb: number[]): number =>
    0.2126 * lin(rgb[0] ?? 0) + 0.7152 * lin(rgb[1] ?? 0) + 0.0722 * lin(rgb[2] ?? 0);
  const parse = (v: string): number[] => (v.match(/\d+(\.\d+)?/g) ?? []).slice(0, 3).map(Number);

  /** Nearest ancestor background that is not see-through. */
  const opaqueBg = (el: Element): number[] => {
    let node: Element | null = el;
    while (node !== null) {
      const parts = (getComputedStyle(node).backgroundColor.match(/[\d.]+/g) ?? []).map(Number);
      if (parts.length >= 3 && (parts[3] ?? 1) > 0.9) return parts.slice(0, 3);
      node = node.parentElement;
    }
    return [255, 255, 255];
  };

  const failures: string[] = [];
  for (const anchor of Array.from(document.querySelectorAll("a"))) {
    const rect = anchor.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) continue;
    const ownText = Array.from(anchor.childNodes).some(
      (node) => node.nodeType === 3 && (node.textContent ?? "").trim().length > 0,
    );
    if (!ownText) continue;

    const style = getComputedStyle(anchor);
    const size = parseFloat(style.fontSize);
    const large = size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700);
    const pair = [lum(parse(style.color)), lum(opaqueBg(anchor))].sort((a, b) => b - a);
    const ratio = ((pair[0] ?? 0) + 0.05) / ((pair[1] ?? 0) + 0.05);
    const floor = large ? 3 : 4.5;
    if (ratio < floor) {
      failures.push(
        `${ratio.toFixed(2)}:1 (need ${floor}) "${(anchor.textContent ?? "").trim().slice(0, 40)}"`,
      );
    }
  }
  return failures;
}

for (const scheme of ["light", "dark"] as const) {
  test(`every link that paints its own text meets AA — ${scheme}`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await unlock(page);
    await selectDraft(page);

    const failures: string[] = [];
    for (const label of PAGES) {
      await navigate(page, label);
      await page.waitForTimeout(350);
      for (const item of await page.evaluate(findFailures)) {
        failures.push(`${label} — ${item}`);
      }
    }
    expect(failures).toEqual([]);
  });
}
