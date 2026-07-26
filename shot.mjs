// Throwaway shell-layout screenshot harness. Run: node shot.mjs <outdir>
import { mkdirSync } from "node:fs";
import { chromium } from "@playwright/test";

const OUT = process.argv[2] ?? "/tmp/shell-shots";
const BASE = "http://127.0.0.1:5173";
const KEY = `mgmt_${"a".repeat(40)}`;
const CSRF = `csrf_${"b".repeat(40)}`;
mkdirSync(OUT, { recursive: true });

async function unlock(page) {
  await page.goto(`${BASE}/#/unlock`);
  await page.getByLabel("Management Key").fill(KEY);
  await page.getByLabel(/CSRF Token/u).fill(CSRF);
  await page.getByRole("button", { name: "解锁" }).click();
  await page.getByRole("heading", { name: "总览" }).waitFor();
}

async function setTheme(page, theme) {
  await page.evaluate((t) => document.documentElement.setAttribute("data-theme", t), theme);
}

/** scroll whichever element actually scrolls (document today, .canvas after) */
async function scrollTo(page, y) {
  await page.evaluate((top) => {
    const canvas = document.querySelector(".canvas");
    if (canvas !== null && canvas.scrollHeight > canvas.clientHeight + 4) {
      canvas.scrollTo({ top, behavior: "instant" });
    } else {
      window.scrollTo({ top, behavior: "instant" });
    }
  }, y);
  await page.waitForTimeout(220);
}

/**
 * Occlusion audit on GLYPH boxes (Range rects), not border boxes — a card
 * whose surface tucks under the rail is fine, its text under the rail is not.
 * `bars` picks which chrome to test: the rail is always checked, the topbar and
 * dock only at rest (scroll top / scroll bottom), where nothing may hide.
 */
async function occlusion(page, label, bars = ["rail"]) {
  const report = await page.evaluate((bars) => {
    const box = (sel) => {
      const el = document.querySelector(sel);
      return el === null ? null : el.getBoundingClientRect();
    };
    const rr = box(".rail");
    const tb = box(".topbar");
    const dk = box(".dock");
    const bad = [];
    const walker = document.createTreeWalker(
      document.querySelector(".canvas"),
      NodeFilter.SHOW_TEXT,
    );
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      const text = (node.textContent ?? "").trim();
      if (text.length === 0) continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const r of range.getClientRects()) {
        if (r.width === 0 || r.height === 0) continue;
        if (r.bottom < 0 || r.top > innerHeight) continue;
        const label = text.slice(0, 22);
        if (bars.includes("rail") && rr !== null &&
            r.left < rr.right - 0.6 && r.bottom > rr.top && r.top < rr.bottom) {
          bad.push({ where: "rail", label, left: +r.left.toFixed(1), railRight: rr.right });
        }
        if (bars.includes("topbar") && tb !== null &&
            r.top < tb.bottom - 0.6 && r.bottom > tb.top && r.left < tb.right && r.right > tb.left) {
          bad.push({ where: "topbar", label, top: +r.top.toFixed(1), barBottom: tb.bottom });
        }
        if (bars.includes("dock") && dk !== null &&
            r.bottom > dk.top + 0.6 && r.top < dk.bottom && r.right > dk.left && r.left < dk.right) {
          bad.push({ where: "dock", label, bottom: +r.bottom.toFixed(1), dockTop: dk.top });
        }
      }
    }
    return bad;
  }, bars);
  if (report.length === 0) console.log(`  occlusion ${label} [${bars}]: CLEAN`);
  else console.log(`  occlusion ${label} [${bars}]: ${report.length} HITS`, JSON.stringify(report.slice(0, 5)));
  return report;
}

async function metrics(page, label) {
  const m = await page.evaluate(() => {
    const q = (s) => document.querySelector(s);
    const r = (s) => {
      const el = q(s);
      return el === null ? null : el.getBoundingClientRect().toJSON();
    };
    const canvas = q(".canvas");
    const cards = [...document.querySelectorAll(".canvas .card")];
    const last = cards.at(-1);
    return {
      scroller: canvas === null ? null : { sh: canvas.scrollHeight, ch: canvas.clientHeight, st: canvas.scrollTop },
      topbar: r(".topbar"),
      rail: r(".rail"),
      dock: r(".dock"),
      lastCard: last === undefined ? null : last.getBoundingClientRect().toJSON(),
      firstCard: cards[0] === undefined ? null : cards[0].getBoundingClientRect().toJSON(),
      docScrollH: document.documentElement.scrollHeight,
    };
  });
  console.log(label, JSON.stringify(m));
  return m;
}

const browser = await chromium.launch();

for (const theme of ["light", "dark"]) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  page.on("console", (msg) => {
    if (msg.type() === "error") console.log(`[console:${theme}]`, msg.text());
  });
  await unlock(page);
  await setTheme(page, theme);
  await page.waitForTimeout(400);

  for (const y of [0, 240, 620, 1400]) {
    await scrollTo(page, y);
    await page.screenshot({ path: `${OUT}/overview-${theme}-y${y}.png` });
    await occlusion(page, `overview/${theme}/y${y}`, y === 0 ? ["rail", "topbar"] : ["rail"]);
  }
  await metrics(page, `overview/${theme}`);

  // table page (access) — first column occlusion check
  await page.getByRole("navigation").getByRole("link", { name: "访问控制", exact: true }).click();
  await page.waitForTimeout(500);
  await scrollTo(page, 0);
  await page.screenshot({ path: `${OUT}/access-${theme}-y0.png` });
  await occlusion(page, `access/${theme}/y0`, ["rail", "topbar"]);
  await scrollTo(page, 260);
  await page.screenshot({ path: `${OUT}/access-${theme}-y260.png` });
  await occlusion(page, `access/${theme}/y260`);
  await metrics(page, `access/${theme}`);

  // horizontally scrolled table: the first column must not slide under the rail
  await page.evaluate(() => {
    const w = document.querySelector(".canvas .tablewrap");
    if (w !== null) w.scrollLeft = 220;
  });
  await page.waitForTimeout(200);
  await page.screenshot({ path: `${OUT}/access-${theme}-tablescrolled.png` });
  await occlusion(page, `access/${theme}/table-scrolled-x`);

  // draft dock over a TALL page: does it cover the last card at max scroll?
  await page.locator(".version-picker select").selectOption("draft-2026-08");
  await page.locator(".dock").waitFor();
  await page.getByRole("navigation").getByRole("link", { name: "总览", exact: true }).click();
  await page.waitForTimeout(600);
  await scrollTo(page, 99999);
  await page.screenshot({ path: `${OUT}/dock-bottom-${theme}.png` });
  const dm = await metrics(page, `dock/${theme}`);
  await occlusion(page, `dock/${theme}/bottom`, ["rail", "dock"]);
  // …and prove the clearance is what saves it: drop it and re-measure
  await page.evaluate(() => document.querySelector(".shell").removeAttribute("data-dock"));
  await scrollTo(page, 99999);
  await page.screenshot({ path: `${OUT}/dock-bottom-${theme}-NOCLEARANCE.png` });
  await occlusion(page, `dock/${theme}/bottom-without-clearance`, ["dock"]);
  await page.evaluate(() => document.querySelector(".shell").setAttribute("data-dock", "true"));
  if (dm.dock !== null && dm.lastCard !== null) {
    const overlap = dm.lastCard.bottom - dm.dock.top;
    console.log(`  >> dock vs last card (${theme}): ${overlap.toFixed(1)}px ${overlap > 0 ? "OCCLUDED" : "clear"}`);
  }
  await ctx.close();
}

// short viewport => real scroll range, so the scroll edge can be seen working
for (const theme of ["light", "dark"]) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 560 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await unlock(page);
  await setTheme(page, theme);
  await page.locator(".version-picker select").selectOption("v-2026-07");
  await page.getByRole("navigation").getByRole("link", { name: "请求监控", exact: true }).click();
  await page.waitForTimeout(800);
  for (const y of [0, 120, 300, 99999]) {
    await scrollTo(page, y);
    await page.screenshot({ path: `${OUT}/scrollrange-${theme}-y${y}.png` });
    await occlusion(page, `scrollrange/${theme}/y${y}`, y === 0 ? ["rail", "topbar"] : ["rail"]);
  }
  await ctx.close();
}

// conflict bar + sheet (portalled) + a11y degradations
for (const [name, media] of [
  ["reduced-transparency", { reducedMotion: "reduce" }],
  ["contrast-more", { contrast: "more" }],
]) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 700 }, deviceScaleFactor: 2, ...media });
  const page = await ctx.newPage();
  await unlock(page);
  await page.emulateMedia(
    name === "contrast-more" ? { contrast: "more" } : { reducedMotion: "reduce" },
  );
  if (name === "reduced-transparency") {
    // Playwright has no prefers-reduced-transparency knob: force the same
    // branch by matching the media query rule manually via CSS var swap.
    await page.evaluate(() => {
      const s = document.createElement("style");
      s.textContent =
        ".glass{background:color-mix(in srgb,var(--surface) 92%,transparent);backdrop-filter:none;-webkit-backdrop-filter:none}" +
        ".canvas{-webkit-mask-image:linear-gradient(to bottom,transparent 0 var(--chrome-bottom),#000 var(--chrome-bottom));mask-image:linear-gradient(to bottom,transparent 0 var(--chrome-bottom),#000 var(--chrome-bottom))}";
      document.head.append(s);
    });
  }
  await scrollTo(page, 200);
  await page.screenshot({ path: `${OUT}/a11y-${name}.png` });
  await occlusion(page, `a11y/${name}`, ["rail"]);
  await ctx.close();
}

// conflict bar present + a sheet open
{
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 800 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await unlock(page);
  await page.locator(".version-picker select").selectOption("draft-2026-08");
  await page.getByRole("navigation").getByRole("link", { name: "访问控制", exact: true }).click();
  await page.waitForTimeout(600);
  await page.evaluate(() => {
    const shell = document.querySelector(".shell");
    shell.setAttribute("data-conflict", "true");
    const deck = document.querySelector(".topdeck");
    const bar = document.createElement("div");
    bar.className = "conflict-bar";
    bar.setAttribute("role", "alert");
    bar.textContent = "配置已被其他会话修改(409),请刷新后重试。";
    deck.append(bar);
  });
  await page.waitForTimeout(200);
  await scrollTo(page, 120);
  await page.screenshot({ path: `${OUT}/conflict-bar.png` });
  await occlusion(page, `conflict/light`, ["rail"]);
  const m = await page.evaluate(() => {
    const c = document.querySelector(".canvas");
    const cs = getComputedStyle(c);
    return { paddingTop: cs.paddingTop, chromeBottom: cs.getPropertyValue("--chrome-bottom") };
  });
  console.log("  conflict canvas padding-top:", JSON.stringify(m));

  // sheet must escape the canvas mask + scroller (portalled to <body>)
  await page.getByRole("button", { name: "签发 Client Key" }).first().click().catch(() => {});
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${OUT}/sheet-open.png` });
  const sheet = await page.evaluate(() => {
    const el = document.querySelector(".sheet-backdrop");
    return el === null ? null : { parent: el.parentElement.tagName, rect: el.getBoundingClientRect().toJSON() };
  });
  console.log("  sheet:", JSON.stringify(sheet));
  await ctx.close();
}

// responsive
for (const theme of ["light", "dark"]) {
  const ctx = await browser.newContext({ viewport: { width: 700, height: 820 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await unlock(page);
  await setTheme(page, theme);
  await page.waitForTimeout(400);
  await scrollTo(page, 0);
  await page.screenshot({ path: `${OUT}/narrow-${theme}-y0.png` });
  await scrollTo(page, 420);
  await page.screenshot({ path: `${OUT}/narrow-${theme}-y420.png` });
  await metrics(page, `narrow/${theme}`);
  await ctx.close();
}

await browser.close();
console.log("shots ->", OUT);
