import {test,expect,type Page} from "@playwright/test";
import {unlock,selectDraft,navigate} from "./helpers";

async function open(page:Page) {
  await unlock(page);await selectDraft(page);await navigate(page,"计费与价格");
  await page.getByRole("button",{name:"导入目录",exact:true}).click();
  const region=page.locator(".inline-workspace");
  await region.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  return region;
}

test("inline dirty switch has one confirmation and retains the edited field",async({page})=>{
  const issues:string[]=[];page.on("console",message=>{if(message.type()==="error"||message.type()==="warning")issues.push(message.text());});
  const editor=await open(page);
  await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"详情",exact:true}).click();
  const confirm=page.getByRole("dialog",{name:"放弃未保存的修改？"});
  await expect(confirm).toHaveCount(1);
  await confirm.getByRole("button",{name:"继续编辑"}).click();
  await expect(editor.getByLabel("生效时间（本地时区）")).toHaveValue("2026-09-19T08:00");
  await expect(editor.getByLabel("生效时间（本地时区）")).toBeFocused();
  await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"详情",exact:true}).click();
  await confirm.getByRole("button",{name:"放弃修改"}).click();
  await expect(editor).toHaveCount(0);
  await expect(page.getByRole("dialog",{name:"价格目录详情"})).toBeVisible();
  expect(issues.filter(value=>/blocker|modal budget/u.test(value))).toEqual([]);
});

test("dock review takes ownership only after accepted discard",async({page})=>{
  const editor=await open(page);
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  const confirm=page.getByRole("dialog",{name:"放弃未保存的修改？"});
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await confirm.getByRole("button",{name:"继续编辑"}).click();
  await expect(editor).toBeVisible();
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  await confirm.getByRole("button",{name:"放弃修改"}).click();
  await expect(editor.getByLabel("生效时间（本地时区）")).toHaveCount(0);
  await expect(page.getByRole("region",{name:"待应用变更",exact:true})).toBeVisible();
});

test("indexed Back can be rejected and then accepted without losing input",async({page})=>{
  const editor=await open(page);
  await page.goBack();
  const confirm=page.getByRole("dialog",{name:"放弃未保存的修改？"});
  await confirm.getByRole("button",{name:"继续编辑"}).click();
  await expect(page).toHaveURL(/#\/billing/u);
  await expect(editor.getByLabel("生效时间（本地时区）")).toHaveValue("2026-09-19T08:00");
  await page.goBack();
  await confirm.getByRole("button",{name:"放弃修改"}).click();
  await expect(editor).toHaveCount(0);
  await expect(page).not.toHaveURL(/#\/billing/u);
});

test("policy and subsequent key edits share one draft until explicit publication",async({page})=>{
  await unlock(page);await navigate(page,"计费与价格");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__pendingCalls",{copies:0,publishes:0});
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const counts=Reflect.get(globalThis,"__pendingCalls") as {copies:number;publishes:number};
      if(operation==="forkConfigVersion")counts.copies++;
      if(operation==="publishConfigVersion")counts.publishes++;
      return original.call(this,operation,request);
    };
  });
  await page.getByText("高级路由价格策略",{exact:true}).click();
  await page.locator(".bill-policy").getByRole("button",{name:/设置策略|更换目录/u}).click();
  const policy=page.getByRole("dialog",{name:"绑定路由价格目录"});
  await policy.getByRole("combobox",{name:"已生效的价格目录"}).selectOption("cat-2026-07");
  await policy.getByRole("button",{name:"保存到草稿"}).click();
  await page.getByRole("dialog",{name:"价格策略结果"}).getByRole("button",{name:"完成"}).click();
  const draft=await page.locator("main").getAttribute("data-context-version");
  await expect(page.locator("main")).toHaveAttribute("data-context-status","draft");
  await navigate(page,"访问控制");
  await page.locator(".key-list tbody tr").filter({hasText:"rgw_9f3c21ab04d7e6b2"}).getByRole("button",{name:"编辑"}).click();
  const key=page.getByRole("dialog",{name:"编辑 API 密钥"});
  await key.getByRole("textbox",{name:"名称"}).fill("Pending batch client");
  await key.getByRole("button",{name:"保存到草稿"}).click();
  const receipt=page.getByRole("dialog",{name:"密钥权限结果"});
  await expect(receipt).toContainText("草稿");
  await receipt.getByRole("button",{name:"核对配置"}).click();
  await expect(page.locator("main")).toHaveAttribute("data-context-version",draft!);
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__pendingCalls"))).toEqual({copies:1,publishes:0});
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  await page.getByRole("region",{name:"待应用变更",exact:true}).getByRole("button",{name:"校验并应用",exact:true}).click();
  await page.getByRole("dialog",{name:"确认应用配置"}).getByRole("button",{name:"确认应用",exact:true}).click();
  await page.getByRole("dialog",{name:"配置操作结果"}).getByRole("button",{name:"完成",exact:true}).click();
  await expect(page.locator("main")).toHaveAttribute("data-context-status","active");
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__pendingCalls"))).toEqual({copies:1,publishes:1});
  expect(await page.evaluate(async()=>{const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");return useVersionStore.getState().pending;})).toBeUndefined();
});

test("a held global write rejects departure without queuing a later Back",async({page})=>{
  await unlock(page);await selectDraft(page);await navigate(page,"计费与价格");
  await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"复制编辑"}).click();
  const editor=page.locator(".inline-workspace");
  await editor.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await editor.getByRole("button",{name:"预览差异"}).click();
  await page.evaluate(async()=>{const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");holdFixtureOperationForTest("POST /admin/billing/catalogs");});
  await editor.getByRole("button",{name:"确认导入"}).click();
  await expect(editor).toHaveAttribute("aria-busy","true");
  await expect(editor.getByRole("button",{name:"关闭编辑"})).toBeDisabled();
  await page.keyboard.press("Escape");
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  await page.goBack();
  await expect(page).toHaveURL(/#\/billing/u);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(async()=>{const {releaseFixtureOperationForTest}=await import("/src/dev/fixtures.ts");releaseFixtureOperationForTest("POST /admin/billing/catalogs");});
  await expect(page.getByRole("region",{name:"目录导入结果"})).toBeVisible();
  await expect(page).toHaveURL(/#\/billing/u);
});

test("same-location navigation is a no-op; same-component query departure retires the editor",async({page})=>{
  const editor=await open(page);
  await page.getByRole("navigation",{name:"工作区页面"}).getByRole("link",{name:"计费与价格",exact:true}).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(editor.getByLabel("生效时间（本地时区）")).toHaveValue("2026-09-19T08:00");
  await page.evaluate(async()=>{const {router}=await import("/src/App.tsx");void router.navigate("/billing?review=synthetic");});
  const confirm=page.getByRole("dialog",{name:"放弃未保存的修改？"});
  await confirm.getByRole("button",{name:"放弃修改"}).click();
  await expect(page).toHaveURL(/#\/billing\?review=synthetic/u);
  await expect(confirm).toHaveCount(0);
  await expect(editor).toHaveCount(0);
});
