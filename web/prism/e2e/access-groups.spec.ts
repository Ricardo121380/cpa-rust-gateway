import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock, selectVersion } from "./helpers";

// Access groups and route grants — the half of the configuration chain Prism
// could not do. Without a group and a grant, an issued Client Key reaches no
// model at all, so these are not conveniences.

async function openAccess(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
}

async function openAdvancedGroups(page: import("@playwright/test").Page): Promise<void> {
  const advanced = page.locator(".access-page details", { hasText: "高级访问组" });
  await advanced.locator("summary").click();
  await expect(advanced.getByRole("button", { name: "新建访问组" })).toBeVisible();
}

test("a group can be created, edited and deleted", async ({ page }) => {
  await openAccess(page);
  await openAdvancedGroups(page);

  await page.getByRole("button", { name: "新建访问组" }).click();
  const create = page.getByRole("dialog");
  await create.getByLabel("访问组标识").fill("team-e2e");
  await create.getByLabel("名称").fill("端到端组");
  await expect(create.locator('input[name="limits"]')).toHaveCount(0);
  await create.getByRole("button", { name: "创建" }).click();

  const row = page.locator('tr:has([data-resource-id="team-e2e"])').first();
  await expect(row).toContainText("端到端组");


  // Edit is a full replacement — the form must arrive pre-filled or the save
  // would silently blank the fields the operator did not touch.
  await row.getByRole("button", { name: "编辑" }).click();
  const edit = page.getByRole("dialog");
  await expect(edit.locator('input[name="id"]')).toHaveValue("team-e2e");
  await expect(edit.getByLabel("名称")).toHaveValue("端到端组");
  await expect(edit.locator('input[name="limits"]')).toHaveCount(0);
  await edit.getByLabel("名称").fill("改名后");
  await edit.getByRole("button", { name: "保存" }).click();
  await expect(page.locator('tr:has([data-resource-id="team-e2e"])').first()).toContainText("改名后");

  await page.locator('tr:has([data-resource-id="team-e2e"])').first().getByRole("button", { name: "删除" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("会同时移除它的路由授权");
  await confirm.getByRole("button", { name: "确认删除" }).click();
  await expect(page.locator('tr:has([data-resource-id="team-e2e"])')).toHaveCount(0);
});

test("legacy limits require explicit clearing or disabled preservation", async ({ page }) => {
  await openAccess(page);
  await page.evaluate(async () => {
    const {call} = await import("/src/api/client.ts");
    await call("createAccessGroup", {body:{id:"legacy-limits",name:"历史限额组",status:"disabled",limits:{rpm:120,max_concurrency:8}}}, {versionScoped:true,mutating:true});
  });
  await navigate(page, "仪表盘");
  await navigate(page, "访问控制");
  await openAdvancedGroups(page);
  const row=page.locator('tr:has([data-resource-id="legacy-limits"])').first();
  await row.getByRole("button",{name:"编辑"}).click();
  const dialog=page.getByRole("dialog");
  await dialog.getByLabel("名称",{exact:true}).fill("保留历史限制");
  await dialog.getByRole("combobox",{name:"状态",exact:true}).selectOption("active");
  await dialog.getByRole("button",{name:"保存",exact:true}).click();
  await expect(dialog.getByRole("alert")).toContainText("请明确清除历史限制");
  await dialog.getByRole("combobox",{name:"状态",exact:true}).selectOption("disabled");
  await dialog.getByRole("button",{name:"保存",exact:true}).click();
  await expect(dialog).toHaveCount(0);
  await expect(row).toContainText("rpm=120");
  await expect(row).toContainText("max_concurrency=8");
  await row.getByRole("button",{name:"编辑"}).click();
  await dialog.getByLabel("清除历史限制").check();
  await dialog.getByRole("combobox",{name:"状态",exact:true}).selectOption("active");
  await dialog.getByRole("button",{name:"保存",exact:true}).click();
  await expect(dialog).toHaveCount(0);
  await expect(row).not.toContainText("rpm=120");
  const saved=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    return (await call<Array<{id:string;limits:Record<string,number>;status:string}>>("listAccessGroups",{},{versionScoped:true})).find(x=>x.id==="legacy-limits");
  });
  expect(saved).toMatchObject({status:"active",limits:{}});
});

test("route grants are listed per group and can be added", async ({ page }) => {
  await openAccess(page);
  await navigate(page, "模型与路由");
  const advancedModels = page.locator("details.models-advanced");
  await advancedModels.locator("summary").click();
  await advancedModels.getByRole("button", { name: "高级模型配置" }).click();
  const model = page.getByRole("dialog");
  await model.getByLabel("模型 ID").fill("pm-grant");
  await model.getByLabel("模型名", { exact: false }).fill("model-grant");
  await model.getByRole("button", { name: "保存" }).click();
  await page.getByRole("dialog",{name:"模型配置结果"}).getByRole("button",{name:"完成"}).click();
  const modelRow = page.locator("tr", { hasText: "model-grant" }).first();
  await modelRow.locator("details.row-menu summary").click();
  await modelRow.getByRole("button", { name: "配置路由" }).click();
  await page.getByRole("dialog").getByLabel("路由标识").fill("rt-e2e");
  await page.getByRole("dialog").getByRole("button", { name: "创建路由" }).click();
  await page.getByRole("dialog",{name:"路由配置结果"}).getByRole("button",{name:"继续配置候选"}).click();
  await navigate(page, "访问控制");
  await openAdvancedGroups(page);


  const row = page.locator('tr:has([data-resource-id="team-default"])').first();
  await row.getByRole("button", { name: "路由" }).click();
  const routes = page.locator(".group-routes");
  await expect(routes).toContainText("rt-minimax");

  await routes.getByRole("button", { name: "授权路由" }).click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("包含未绑定草稿路由");
  await expect(sheet.locator('select[name="route_id"] option[value="rt-e2e"]')).toHaveCount(1);
  await sheet.getByRole("combobox", {name:"路由"}).selectOption("rt-e2e");
  await sheet.getByRole("button", { name: "授权" }).click();
  await expect(page.locator('.group-routes [data-resource-id="rt-e2e"]')).toBeVisible();
});

test("a group with no grant says the keys under it reach nothing", async ({ page }) => {
  await openAccess(page);
  await openAdvancedGroups(page);
  // team-batch exists with no grants seeded
  await page.locator('tr:has([data-resource-id="team-batch"])').first().getByRole("button", { name: "路由" }).click();
  await expect(page.locator(".group-routes")).toContainText("到不了任何模型");
});

test("route grant recovery restarts a failed first page",async({page})=>{
  await openAccess(page);
  await page.evaluate(async()=>{const {call}=await import("/src/api/client.ts");await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-recovery",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});});
  await openAdvancedGroups(page);
  await page.locator('tr:has([data-resource-id="team-default"])').first().getByRole("button",{name:"路由"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__routeGrantReads",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="listRoutes"){
        const reads=Number(Reflect.get(globalThis,"__routeGrantReads"))+1;
        Reflect.set(globalThis,"__routeGrantReads",reads);
        if(reads===1)throw new Error("synthetic first route page failure");
      }
      return original.call(this,operation,request);
    };
  });
  await page.locator(".group-routes").getByRole("button",{name:"授权路由"}).click();
  const sheet=page.getByRole("dialog",{name:/授权路由/u});
  await expect(sheet.getByRole("alert")).toContainText("synthetic first route page failure");
  await sheet.getByRole("button",{name:"重新读取路由"}).click();
  await expect(sheet.getByRole("alert")).toHaveCount(0);
  await expect(sheet.getByRole("combobox",{name:"路由"}).locator('option[value="rt-recovery"]')).toHaveCount(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__routeGrantReads")))).toBe(2);
});

test("route grant recovery discards a rejected continuation cursor",async({page})=>{
  await openAccess(page);
  await page.evaluate(async()=>{const {call}=await import("/src/api/client.ts");await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-recovery",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});});
  await openAdvancedGroups(page);
  await page.locator('tr:has([data-resource-id="team-default"])').first().getByRole("button",{name:"路由"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__routeGrantFirstReads",0);
    Reflect.set(globalThis,"__routeGrantCursorReads",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="listRoutes")return original.call(this,operation,request);
      const cursor=(request as {query?:{cursor?:string}}).query?.cursor;
      if(cursor){Reflect.set(globalThis,"__routeGrantCursorReads",Number(Reflect.get(globalThis,"__routeGrantCursorReads"))+1);throw new Error("synthetic rejected cursor");}
      const reads=Number(Reflect.get(globalThis,"__routeGrantFirstReads"))+1;
      Reflect.set(globalThis,"__routeGrantFirstReads",reads);
      const response=await original.call(this,operation,request);
      if(reads!==1)return response;
      const page=await response.clone().json() as Record<string,unknown>;
      return new Response(JSON.stringify({...page,next_cursor:"synthetic-cursor"}),{status:response.status,headers:response.headers});
    };
  });
  await page.locator(".group-routes").getByRole("button",{name:"授权路由"}).click();
  const sheet=page.getByRole("dialog",{name:/授权路由/u});
  await sheet.getByRole("button",{name:"加载更多路由"}).click();
  await expect(sheet.getByRole("alert")).toContainText("synthetic rejected cursor");
  await sheet.getByRole("button",{name:"重新读取路由"}).click();
  await expect(sheet.getByRole("alert")).toHaveCount(0);
  await expect(sheet.getByRole("combobox",{name:"路由"}).locator('option[value="rt-recovery"]')).toHaveCount(1);
  expect(await page.evaluate(()=>({first:Number(Reflect.get(globalThis,"__routeGrantFirstReads")),cursor:Number(Reflect.get(globalThis,"__routeGrantCursorReads"))}))).toEqual({first:2,cursor:1});
});

test("group editing is refused on a published version", async ({ page }) => {
  await unlock(page);
  await navigate(page, "访问控制");
  // v-2026-07 is active, not a draft
  await selectVersion(page, "v-2026-07");
  await openAdvancedGroups(page);
  await expect(page.getByRole("button", { name: "新建访问组" })).toBeDisabled();
});
