import {expect,test,type Page} from "@playwright/test";
import {navigate,selectDraft,unlock} from "./helpers";
async function approve(page:Page) {
  const id=await page.locator('[data-native-session-id]').getAttribute('data-native-session-id');
  if(!id)throw new Error('missing session');
  await page.evaluate(async(session)=>{
    const fixture=await import('/src/dev/fixtures.ts');
    fixture.approveNativeDeviceForTest(session);
  },id);
}
test("native SSO import is immediately listed without a provider binding",async({page})=>{
  await unlock(page);await selectDraft(page);await navigate(page,"账号池");
  await page.getByRole('button',{name:'添加账号',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'添加账号'});
  await dialog.getByLabel('渠道',{exact:true}).selectOption('grok.console');
  await dialog.getByLabel('账号名称').fill('Console 团队账号');
  await dialog.locator('textarea').fill(JSON.stringify({sso_token:'synthetic-sso',probe_model:'grok-4.6'}));
  await dialog.getByRole('button',{name:'添加账号',exact:true}).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('region',{name:'Grok 账号',exact:true})).toContainText('Console 团队账号');
  await expect(page.getByRole('region',{name:'Grok 账号',exact:true})).not.toContainText('synthetic-sso');
});
test("Grok first authorization waits for provider consent, then reauthorizes the same account",async({page})=>{
  await unlock(page);await selectDraft(page);await navigate(page,"账号池");
  await page.getByRole('button',{name:'添加账号',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'添加账号',exact:true});
  await dialog.getByLabel('渠道',{exact:true}).selectOption('grok.build');
  await dialog.getByLabel('账号名称').fill('Build 授权账号');
  await dialog.getByRole('button',{name:'授权登录',exact:true}).click();
  await page.getByRole('button',{name:'开始 Grok 授权',exact:true}).click();
  await expect(page.getByRole('link',{name:'打开 Grok 授权页面'})).toBeVisible();
  await expect(page.locator('[data-native-session-id]')).toHaveText('等待 Grok 授权');
  await expect(page.locator('[data-account-key^=grok-]')).toHaveCount(0);
  await approve(page);
  await expect(page.locator('[data-native-session-id]')).toHaveText('授权已保存');
  await page.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(page.locator('[data-account-key^=grok-]')).toHaveCount(1);
  const firstId=await page.locator('[data-account-key^=grok-]').getAttribute('data-account-key');
  await page.getByRole('region',{name:'Grok 账号',exact:true}).getByRole('button',{name:'重新授权',exact:true}).click();
  await page.getByRole('button',{name:'开始 Grok 授权',exact:true}).click();
  await expect(page.locator('[data-native-session-id]')).toHaveText('等待 Grok 授权');
  await approve(page);
  await expect(page.locator('[data-native-session-id]')).toHaveText('授权已保存');
  await page.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(page.locator('[data-account-key^=grok-]')).toHaveCount(1);
  await expect(page.locator('[data-account-key^=grok-]')).toHaveAttribute('data-account-key',firstId!);
});
