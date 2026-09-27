import {expect,test} from "@playwright/test";
import {navigate,selectDraft,unlock} from "./helpers";

test("account row deletion requires confirmation and leaves the refreshed directory usable",async({page})=>{
  await unlock(page);await selectDraft(page);await navigate(page,"账号池");
  const row=page.locator(".account-list tbody tr").filter({hasText:"alex@example.test"});
  await row.getByRole("button",{name:"删除",exact:true}).click();
  let dialog=page.getByRole("dialog",{name:"删除 1 份授权"});
  await expect(dialog).toContainText("历史请求与账本保留");
  await dialog.getByRole("button",{name:"取消",exact:true}).click();
  await expect(row).toHaveCount(1);
  await row.getByRole("button",{name:"删除",exact:true}).click();
  dialog=page.getByRole("dialog",{name:"删除 1 份授权"});
  await dialog.getByRole("button",{name:"确认删除",exact:true}).click();
  await expect(dialog).toContainText("已保存到草稿");
  await dialog.getByRole("button",{name:"完成",exact:true}).click();
  await expect(row).toHaveCount(0);
  await expect(page.getByRole("button",{name:"刷新快照",exact:true})).toBeEnabled();
});

test("native account has the same direct deletion confirmation and refresh",async({page})=>{
  await unlock(page);await selectDraft(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("importNativeAccount",{body:{id:"deletion-fixture",channel:"grok.console",secret:"synthetic-fixture-only"}});
  });
  await navigate(page,"账号池");
  const row=page.locator(".account-list tbody tr").filter({hasText:"session.member@example.test"});
  await row.getByRole("button",{name:"删除",exact:true}).click();
  const dialog=page.getByRole("dialog",{name:"删除 1 份授权"});
  await expect(dialog).toContainText("Grok Console");
  await dialog.getByRole("button",{name:"确认删除",exact:true}).click();
  await expect(dialog).toContainText("已应用");
  await dialog.getByRole("button",{name:"完成",exact:true}).click();
  await expect(row).toHaveCount(0);
  await expect(page.getByRole("button",{name:"刷新快照",exact:true})).toBeEnabled();
});
