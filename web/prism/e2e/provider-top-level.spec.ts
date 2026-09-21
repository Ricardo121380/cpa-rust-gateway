import {test,expect,type Page} from "@playwright/test";
import {unlock,navigate,selectDraft} from "./helpers";

async function activeProvider(page:Page){
 await unlock(page);await selectDraft(page);await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
 await page.getByRole("region",{name:"待应用变更",exact:true}).getByRole("button",{name:"校验并应用",exact:true}).click();
 await page.getByRole("dialog",{name:"确认应用配置"}).getByRole("button",{name:"确认应用",exact:true}).click();
 await page.getByRole("dialog",{name:"配置操作结果"}).getByRole("button",{name:"完成",exact:true}).click();await navigate(page,"上游");
}

test("provider edit saves one pending draft with stable footer and no early publication",async({page})=>{
 await activeProvider(page);
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;Reflect.set(globalThis,"__providerPublishes",0);ManagementApi.prototype.request=async function(this:unknown,op:string,request:unknown){if(op==="publishConfigVersion")Reflect.set(globalThis,"__providerPublishes",Number(Reflect.get(globalThis,"__providerPublishes"))+1);return original.call(this,op,request);};});
 await page.locator(".provider-card",{hasText:"中转站 A"}).getByRole("button",{name:"编辑",exact:true}).click();
 const edit=page.getByRole("dialog");await edit.getByLabel("名称",{exact:true}).fill("Production provider");
 await expect(edit.locator(".sheet-footer").getByRole("button",{name:"保存到草稿"})).toBeVisible();
 await edit.getByRole("button",{name:"保存到草稿"}).click();
 const receipt=page.getByRole("dialog",{name:"提供商修改结果"});await expect(receipt).toContainText("尚未应用");
 expect(await page.evaluate(()=>Reflect.get(globalThis,"__providerPublishes"))).toBe(0);
 await receipt.getByRole("button",{name:"查看工作草稿"}).click();
 await expect(page.locator(".provider-card",{hasText:"Production provider"})).toBeVisible();
 await expect(page.locator(".dock")).toContainText("待应用草稿");
});

test("lost provider update cannot be replayed from its form",async({page})=>{
 await activeProvider(page);
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;Reflect.set(globalThis,"__providerWrites",0);ManagementApi.prototype.request=async function(this:unknown,op:string,request:unknown){const response=await original.call(this,op,request);if(op==="updateUpstream"){Reflect.set(globalThis,"__providerWrites",Number(Reflect.get(globalThis,"__providerWrites"))+1);throw new Error("synthetic response lost");}return response;};});
 await page.locator(".provider-card",{hasText:"中转站 A"}).getByRole("button",{name:"编辑",exact:true}).click();
 const edit=page.getByRole("dialog");await edit.getByLabel("名称",{exact:true}).fill("Lost response provider");await edit.getByRole("button",{name:"保存到草稿"}).click();
 await expect(edit).toContainText("synthetic response lost");await expect(edit.getByRole("button",{name:"保存到草稿"})).toBeDisabled();
 await edit.getByRole("button",{name:"查看待应用的修改"}).click();
 await expect(page.locator(".provider-card",{hasText:"Lost response provider"})).toBeVisible();
 expect(await page.evaluate(()=>Reflect.get(globalThis,"__providerWrites"))).toBe(1);
});

test("provider removal remains pending and uses fixed confirmation actions",async({page})=>{
 await activeProvider(page);
 const provider=page.locator(".provider-card",{hasText:"中转站 A"});await provider.locator("summary").click();await provider.getByRole("button",{name:"移除提供商",exact:true}).click();
 const confirm=page.getByRole("dialog",{name:"移除提供商"});await expect(confirm.locator(".sheet-footer").getByRole("button",{name:"确认删除"})).toBeVisible();
 await confirm.getByRole("button",{name:"确认删除"}).click();
 const receipt=page.getByRole("dialog",{name:"提供商修改结果"});await expect(receipt).toContainText("尚未应用");await receipt.getByRole("button",{name:"查看工作草稿"}).click();
 await expect(page.locator(".provider-card",{hasText:"中转站 A"})).toHaveCount(0);await expect(page.locator("main")).toHaveAttribute("data-context-status","draft");
});

test("provider creation is inline, validates before admission and retains pending receipt",async({page})=>{
 await activeProvider(page);
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;Reflect.set(globalThis,"__newPublishes",0);ManagementApi.prototype.request=async function(this:unknown,op:string,request:unknown){if(op==="publishConfigVersion")Reflect.set(globalThis,"__newPublishes",Number(Reflect.get(globalThis,"__newPublishes"))+1);return original.call(this,op,request);};});
 await page.getByRole("button",{name:"添加提供商",exact:true}).click();
 const editor=page.getByRole("region",{name:"添加 AI 提供商",exact:true});
 await expect(page.getByRole("dialog")).toHaveCount(0);
 await editor.getByLabel("名称",{exact:true}).fill("New provider");
 await editor.getByLabel("接口地址",{exact:true}).fill("http://example.com");
 await editor.getByRole("button",{name:"保存到草稿"}).click();await expect(editor).toContainText("接口地址应为 HTTPS");
 await expect(editor.getByRole("button",{name:"保存到草稿"})).toBeEnabled();
 await editor.getByLabel("接口地址",{exact:true}).fill("https://example.com/v1");
 await editor.getByRole("button",{name:"保存到草稿"}).click();
 const receipt=page.getByRole("region",{name:"提供商创建结果",exact:true});await expect(receipt).toContainText("尚未应用");
 expect(await page.evaluate(()=>Reflect.get(globalThis,"__newPublishes"))).toBe(0);
 await receipt.getByRole("button",{name:"查看工作草稿"}).click();
 await expect(page.locator(".provider-card",{hasText:"New provider"})).toContainText("example.com");
});

test("creation owns dirty departure and uncertain creation cannot replay",async({page})=>{
 await activeProvider(page);await page.getByRole("button",{name:"添加提供商",exact:true}).click();
 const editor=page.getByRole("region",{name:"添加 AI 提供商",exact:true});await editor.getByLabel("名称",{exact:true}).fill("Uncertain provider");
 await page.locator(".provider-card",{hasText:"中转站 A"}).getByRole("button",{name:"编辑",exact:true}).click();
 await page.getByRole("dialog",{name:"放弃未保存的修改？"}).getByRole("button",{name:"继续编辑"}).click();
 await expect(editor.getByLabel("名称",{exact:true})).toHaveValue("Uncertain provider");
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;Reflect.set(globalThis,"__newWrites",0);ManagementApi.prototype.request=async function(this:unknown,op:string,request:unknown){const result=await original.call(this,op,request);if(op==="createUpstream"){Reflect.set(globalThis,"__newWrites",Number(Reflect.get(globalThis,"__newWrites"))+1);throw new Error("synthetic creation receipt lost");}return result;};});
 await editor.getByRole("button",{name:"保存到草稿"}).click();await expect(editor).toContainText("synthetic creation receipt lost");
 await expect(editor.getByRole("button",{name:"保存到草稿"})).toBeDisabled();
 await editor.getByRole("button",{name:"查看待应用的修改"}).click();
 await expect(page.locator(".provider-card",{hasText:"Uncertain provider"})).toBeVisible();
 expect(await page.evaluate(()=>Reflect.get(globalThis,"__newWrites"))).toBe(1);
});

test("pending provider creation blocks close and local action without replaying navigation",async({page})=>{
 await activeProvider(page);await page.getByRole("button",{name:"添加提供商",exact:true}).click();
 const editor=page.getByRole("region",{name:"添加 AI 提供商",exact:true});await editor.getByLabel("名称",{exact:true}).fill("Held provider");
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;ManagementApi.prototype.request=async function(this:unknown,op:string,request:unknown){if(op==="createUpstream")await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseProvider",resolve));return original.call(this,op,request);};});
 await editor.getByRole("button",{name:"保存到草稿"}).click();await expect(editor.getByRole("button",{name:"取消"})).toBeDisabled();
 await page.waitForFunction(()=>typeof Reflect.get(globalThis,"__releaseProvider")==="function");
 await page.keyboard.press("Escape");await page.locator(".provider-card",{hasText:"中转站 A"}).getByRole("button",{name:"编辑",exact:true}).click();
 await expect(page.getByRole("dialog")).toHaveCount(0);await expect(editor).toBeVisible();
 await page.evaluate(()=>Reflect.get(globalThis,"__releaseProvider")());await expect(page.getByRole("region",{name:"提供商创建结果",exact:true})).toBeVisible();await expect(page.getByRole("dialog")).toHaveCount(0);
});
