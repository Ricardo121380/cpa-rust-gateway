import {test,expect} from "@playwright/test";
import {unlock,selectDraft,navigate} from "./helpers";

test("inline editors keep balanced gutters and reachable controls at target sizes",async({page})=>{
 await unlock(page);await selectDraft(page);await navigate(page,"出口策略");await page.getByRole("button",{name:"新建出口策略",exact:true}).click();
 for(const size of [{width:1440,height:900},{width:1280,height:720},{width:390,height:844}]){
  await page.setViewportSize(size);
  const metrics=await page.locator('.inline-workspace-body').evaluate(element=>{const style=getComputedStyle(element);return {left:parseFloat(style.paddingLeft),right:parseFloat(style.paddingRight),overflow:document.documentElement.scrollWidth>innerWidth};});
  expect(metrics.left).toBeGreaterThanOrEqual(16);expect(metrics.left).toBe(metrics.right);expect(metrics.overflow).toBe(false);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  for(const button of await page.locator('.inline-workspace-footer button').all())expect((await button.boundingBox())!.height).toBeGreaterThanOrEqual(44);
 }
 await page.emulateMedia({reducedMotion:'reduce'});
 await expect(page.getByRole('region',{name:'新建出口策略',exact:true})).toBeVisible();
});

test("mobile maintenance form preserves labels and readable input sizing",async({page})=>{
 await page.setViewportSize({width:390,height:844});await unlock(page);await selectDraft(page);await navigate(page,"上游");await page.locator('.provider-card',{hasText:'中转站 A'}).getByRole('button',{name:'编辑',exact:true}).click();
 const dialog=page.getByRole('dialog');await expect(dialog.getByLabel('名称',{exact:true})).toHaveCSS('font-size','16px');
 await expect(dialog.locator('fieldset')).toHaveCSS('border-top-width','0px');
 await expect(dialog.getByRole('button',{name:'保存到草稿',exact:true})).toBeVisible();
 const edges=await dialog.boundingBox();expect(edges!.x).toBeGreaterThanOrEqual(12);expect(edges!.x+edges!.width).toBeLessThanOrEqual(378);
});

test("inline cancellation restores the opener without stealing route focus", async ({ page }) => {
 await unlock(page);await navigate(page,"上游");
 const opener=page.getByRole("button",{name:"添加提供商",exact:true});
 await opener.click();
 await expect(page.getByRole("heading",{name:"添加 AI 提供商",exact:true})).toBeFocused();
 await page.keyboard.press("Escape");
 await expect(page.locator(".inline-workspace")).toHaveCount(0);
 await expect(opener).toBeFocused();
 await opener.click();
 await page.locator(".inline-workspace").getByRole("button",{name:"取消",exact:true}).click();
 await expect(opener).toBeFocused();
 await opener.click();
 await navigate(page,"模型与路由");
 await expect(page.locator(".inline-workspace")).toHaveCount(0);
 await expect(page.locator("#main-navigation").getByRole("link",{name:"模型管理",exact:true})).toBeFocused();
});
