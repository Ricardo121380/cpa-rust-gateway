import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

for(const width of [1440,1280,390])test(`provider detail selection and return focus at ${width}`,async({page})=>{
  await page.setViewportSize({width,height:900});
  await unlock(page);await selectDraft(page);await navigate(page,"上游");
  const provider=page.locator("article",{hasText:"中转站 A"});
  await expect(provider.getByRole("link",{name:"目录与模型开放"})).toHaveCount(1);
  const opener=provider.getByRole("button",{name:"接口与账号",exact:true});
  await opener.focus();await page.keyboard.press("Enter");
  const detail=page.getByRole("complementary",{name:"提供商接口与账号"});
  await expect(detail).toBeFocused();
  await expect(provider).toHaveAttribute("data-selected","true");
  await expect(detail.locator(".subresource-panel")).toBeVisible();
  await expect(detail.getByRole("alert")).toHaveCount(0);
  if(width===390){
    const position=await page.evaluate(()=>({detail:document.querySelector('.provider-detail')!.getBoundingClientRect().top,list:document.querySelector('.provider-list')!.getBoundingClientRect().top,overflow:document.documentElement.scrollWidth>innerWidth}));
    expect(position.detail).toBeLessThan(position.list);expect(position.overflow).toBe(false);
  }
  await detail.getByRole("button",{name:"关闭",exact:true}).click();
  await expect(provider.getByRole("button",{name:"接口与账号",exact:true})).toBeFocused();
  await expect(detail).toHaveCount(0);
});
