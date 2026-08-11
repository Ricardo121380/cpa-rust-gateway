// Lab screenshot harness for proto/glass-lab.html.
// usage: node proto/shoot.mjs [outdir] [mode]   mode = full | crop
import { chromium } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join, resolve } from "node:path";

const OUT = resolve(process.argv[2] ?? "/tmp/glasslab");
const MODE = process.argv[3] ?? "full";
mkdirSync(OUT, { recursive: true });
const url = pathToFileURL(resolve("proto/glass-lab.html")).href;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
page.on("console", (m) => { if (m.type() === "error") console.log("[err]", m.text()); });
await page.goto(url);
await page.waitForFunction(() => window.__labReady === true);
await page.waitForTimeout(500);
console.log("probe:", await page.textContent("#lensState"));

async function set(theme, treatment) {
  await page.evaluate(([t, tr]) => {
    document.documentElement.dataset.theme = t;
    document.documentElement.dataset.treatment = tr;
    document.documentElement.dataset.lab = "off";
    window.dispatchEvent(new Event("resize"));
  }, [theme, treatment]);
  await page.waitForTimeout(400);
}
async function scroll(y) {
  await page.evaluate((v) => {
    const c = document.querySelector(".canvas");
    c.scrollTo({ top: v, behavior: "instant" });
    c.dispatchEvent(new Event("scroll"));
  }, y);
  await page.waitForTimeout(340);
}

if (MODE === "full") {
  for (const theme of ["light", "dark"]) {
    for (const tr of ["a", "b", "c"]) {
      for (const y of [0, 430, 900]) {
        await set(theme, tr);
        await scroll(y);
        await page.screenshot({ path: join(OUT, `${theme}-${tr}-y${y}.png`) });
      }
    }
  }
  console.log("full done");
} else {
  for (const theme of ["light", "dark"]) {
    await set(theme, "c");
    await page.evaluate(() => document.querySelector(".compare").scrollIntoView({ block: "center" }));
    await page.waitForTimeout(400);
    const box = await page.locator(".compare").boundingBox();
    await page.screenshot({
      path: join(OUT, `compare-${theme}.png`),
      clip: { x: box.x - 4, y: box.y - 4, width: box.width + 8, height: box.height + 8 },
    });
    for (const tr of ["a", "b", "c"]) {
      await set(theme, tr);
      await page.evaluate(() => { const c=document.querySelector(".canvas"); c.scrollTo({top: document.querySelector(".compare").offsetTop - 40, behavior:"instant"}); c.dispatchEvent(new Event("scroll")); });
      await page.waitForTimeout(400);
      await page.screenshot({ path: join(OUT, `topbar-${theme}-${tr}.png`), clip: { x: 0, y: 0, width: 1000, height: 190 } });
      await page.screenshot({ path: join(OUT, `rail-${theme}-${tr}.png`), clip: { x: 0, y: 60, width: 420, height: 560 } });
      await page.screenshot({ path: join(OUT, `dock-${theme}-${tr}.png`), clip: { x: 380, y: 740, width: 760, height: 160 } });
    }
  }
  console.log("crop done");
}
await browser.close();
