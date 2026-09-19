import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("legacy account labels stay readable while copying and operations retain exact IDs", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async () => {
    const path = "/src/generated/management-client.ts";
    const { ManagementApi } = await import(path);
    const original = ManagementApi.prototype.request;
    const state = window as unknown as { copied?: string; action?: unknown };
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
      writeText: async (value: string) => { state.copied = value; },
    } });
    ManagementApi.prototype.request = async function(this: unknown, operation: string, request: { body?: unknown }) {
      if (operation === "applyProviderAccountPoolAction") state.action = request.body;
      const response = await original.call(this, operation, request);
      if (operation !== "listProviderAccountPools") return response;
      const data = await response.json();
      data.items = [6, 9].map((phase) => ({ ...data.items[0],
        provider_id: "p12-06-codex-bridge-upstream",
        channel_id: "p12-06-codex-bridge-endpoint",
        account_id: `p12-${String(phase).padStart(2, "0")}-codex-bridge-credential`,
        presentation: {...data.items[0].presentation,provider:"Codex",category:"codex",identity:{email:`member-${phase}@example.test`,phone:null,username:null}},
      }));
      return new Response(JSON.stringify(data), { status: response.status, headers: response.headers });
    };
  });
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await expect(page.locator(".account-desktop tbody tr")).toHaveCount(2);
  expect(await page.locator(".account-desktop").innerText()).not.toContain("p12-");
  const id = "p12-09-codex-bridge-credential";
  const row = page.locator(".account-desktop tbody tr").filter({ hasText: "member-9@example.test" });
  await expect(row).toContainText("member-9@example.test");
  await expect(page.locator(".account-desktop .resource-code")).toHaveCount(0);
  await row.getByRole("button", { name: "详情", exact: true }).click();
  const detail = page.getByRole("dialog");
  await detail.getByText("关联信息", { exact: true }).click();
  await detail.getByRole("button", { name: "复制账号内部引用", exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as { copied: string }).copied)).toBe(id);
  await detail.getByRole("button", { name: "冷却账号" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认冷却" }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as { action: unknown }).action)).toMatchObject({
    account_id: id, provider_id: "p12-06-codex-bridge-upstream", channel_id: "p12-06-codex-bridge-endpoint",
  });
});

test("a native runtime account sharing an ordinary ID never opens that ordinary credential", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__runtimeCredentialReads", 0);
    ManagementApi.prototype.request = async function(this: unknown, operation: string, request: unknown) {
      if (operation === "getCredential") Reflect.set(globalThis, "__runtimeCredentialReads", Number(Reflect.get(globalThis, "__runtimeCredentialReads")) + 1);
      const response = await original.call(this, operation, request);
      if (operation !== "listProviderAccountPools") return response;
      const data = await response.json();
      data.items = [{ ...data.items[0],
        provider_id: "grok-build-pool", channel_id: "ep-grok-build", account_id: "cred-relay-key", account_kind: "grok_build_oauth",
        presentation: {...data.items[0].presentation,provider:"Grok Build",category:"grok",identity:{email:"native.member@example.test",phone:null,username:null}},
      }];
      return new Response(JSON.stringify(data), { status: response.status, headers: response.headers });
    };
  });
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await page.locator(".account-desktop").getByRole("button", {name:"详情",exact:true}).click();
  const detail=page.getByRole("dialog");
  await detail.getByRole("button", {name:"配置与失败",exact:true}).click();
  await detail.getByRole("button", {name:"查看凭据与绑定",exact:true}).click();
  await expect(page.getByRole("dialog", {name:"未建立精确凭据映射"})).toBeVisible();
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__runtimeCredentialReads")))).toBe(0);
});

for (const failure of [{status:409,code:"management_native_account_conflict"},{status:503,code:"fixture_native_inventory_unavailable"}] as const) test(`a cached native match cannot open after a ${failure.status} inventory failure`, async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async ({status,code}) => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__nativeInventoryCalls", 0);
    Reflect.set(globalThis, "__runtimeCredentialReads", 0);
    ManagementApi.prototype.request = async function(this: unknown, operation: string, request: unknown) {
      if (operation === "getCredential") Reflect.set(globalThis, "__runtimeCredentialReads", Number(Reflect.get(globalThis, "__runtimeCredentialReads")) + 1);
      if (operation === "listNativeAccounts") {
        const count = Number(Reflect.get(globalThis, "__nativeInventoryCalls")) + 1;
        Reflect.set(globalThis, "__nativeInventoryCalls", count);
        if (count === 2) return new Response(JSON.stringify({error:{code,message:"synthetic later page failure"}}), {status,headers:{"Content-Type":"application/json"}});
        return new Response(JSON.stringify({items:[{id:"cred-relay-key",provider:"grok_build",auth_status:"active",enabled:true,revision:7,import_batch_id:"native-match",identity:{email:"native.member@example.test",phone:null,username:null}}],next_cursor:"second-page"}), {status:200,headers:{"Content-Type":"application/json"}});
      }
      const response = await original.call(this, operation, request);
      if (operation !== "listProviderAccountPools") return response;
      const data = await response.json();
      data.items = [{...data.items[0],provider_id:"grok-build-pool",channel_id:"ep-grok-build",account_id:"cred-relay-key",account_kind:"grok_build_oauth",presentation:{...data.items[0].presentation,provider:"Grok Build",category:"grok",identity:{email:"native.member@example.test",phone:null,username:null}}}];
      return new Response(JSON.stringify(data), {status:response.status,headers:response.headers});
    };
  }, failure);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name:"账号视图"}).getByRole("button", {name:"运行状态",exact:true}).click();
  await page.locator(".account-desktop").getByRole("button", {name:"详情",exact:true}).click();
  const runtime = page.getByRole("dialog");
  await runtime.getByRole("button", {name:"配置与失败",exact:true}).click();
  await runtime.getByRole("button", {name:"查看凭据与绑定",exact:true}).click();
  await expect(page.getByRole("dialog", {name:"未建立精确凭据映射"})).toContainText("synthetic later page failure");
  await expect(page.getByRole("dialog", {name:"账号详情"})).toHaveCount(0);
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__nativeInventoryCalls")))).toBe(2);
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__runtimeCredentialReads")))).toBe(0);
  await page.getByRole("dialog", {name:"未建立精确凭据映射"}).getByRole("button", {name:"关闭",exact:true}).click();
  await expect(page.getByRole("dialog", {name:"未建立精确凭据映射"})).toHaveCount(0);
});

for (const failure of [{status:409,code:"management_native_account_conflict"},{status:503,code:"fixture_native_inventory_unavailable"}] as const) test(`a native inventory ${failure.status} page leaves an explicit boundary without retry`, async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async ({status,code}) => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__nativeInventoryCalls", 0);
    Reflect.set(globalThis, "__nativeInventoryCursors", []);
    ManagementApi.prototype.request = async function(this: unknown, operation: string, request: {query?:{cursor?:string}}) {
      if (operation === "listNativeAccounts") {
        const count=Number(Reflect.get(globalThis, "__nativeInventoryCalls"))+1;
        Reflect.set(globalThis, "__nativeInventoryCalls", count);
        (Reflect.get(globalThis, "__nativeInventoryCursors") as Array<string|null>).push(request.query?.cursor??null);
        if(count===2)return new Response(JSON.stringify({error:{code,message:"synthetic native inventory failure"}}), {status,headers:{"Content-Type":"application/json"}});
        if(count===3)return new Response(JSON.stringify({items:[],next_cursor:"fresh-page"}), {status:200,headers:{"Content-Type":"application/json"}});
        if(count===4)return new Response(JSON.stringify({items:[{id:"native-page-two",provider:"grok_build",auth_status:"active",enabled:true,revision:8,import_batch_id:"native-page-two",identity:{email:"native.fresh@example.test",phone:null,username:null}}],next_cursor:null}), {status:200,headers:{"Content-Type":"application/json"}});
        const response=await original.call(this, operation, request);
        const body=await response.json();
        return new Response(JSON.stringify({...body,items:[],next_cursor:"rejected-page"}), {status:response.status,headers:response.headers});
      }
      const response = await original.call(this, operation, request);
      if (operation !== "listProviderAccountPools") return response;
      const data = await response.json();
      data.items = [{ ...data.items[0], provider_id:"grok-build-pool",channel_id:"ep-grok-build",account_id:"native-page-two",account_kind:"grok_build_oauth",presentation:{...data.items[0].presentation,provider:"Grok Build",category:"grok",identity:{email:"native.page@example.test",phone:null,username:null}} }];
      return new Response(JSON.stringify(data), { status: response.status, headers: response.headers });
    };
  }, failure);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name:"账号视图"}).getByRole("button", {name:"运行状态",exact:true}).click();
  await page.locator(".account-desktop").getByRole("button", {name:"详情",exact:true}).click();
  const runtime=page.getByRole("dialog");
  await runtime.getByRole("button", {name:"配置与失败",exact:true}).click();
  await runtime.getByRole("button", {name:"查看凭据与绑定",exact:true}).click();
  const boundary=page.getByRole("dialog", {name:"未建立精确凭据映射"});
  await expect(boundary).toContainText("synthetic native inventory failure");
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__nativeInventoryCalls")))).toBe(2);
  expect(await page.evaluate(() => Reflect.get(globalThis, "__nativeInventoryCursors"))).toEqual([null,"rejected-page"]);
  if(failure.status===409){
    await boundary.getByRole("button", {name:"重新读取原生账号",exact:true}).click();
    await expect(page.getByRole("dialog", {name:"账号详情"})).toContainText("native.fresh@example.test");
    expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__nativeInventoryCalls")))).toBe(4);
    expect(await page.evaluate(() => Reflect.get(globalThis, "__nativeInventoryCursors"))).toEqual([null,"rejected-page",null,"fresh-page"]);
  }
});

for(const failedRead of ["transport","missing_binding"] as const)test(`a ${failedRead} ordinary binding reread cannot reopen cached maintenance`, async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"账号池");
  await page.getByRole("navigation",{name:"账号视图"}).getByRole("button",{name:"运行状态",exact:true}).click();
  const openConfiguration=async()=>{
    await page.getByRole("textbox",{name:"搜索已加载账号"}).fill("cred-relay-key");
    await page.locator(".account-desktop").getByRole("button",{name:"详情",exact:true}).click();
    const runtime=page.getByRole("dialog");
    await runtime.getByRole("button",{name:"配置与失败",exact:true}).click();
    await runtime.getByRole("button",{name:"查看凭据与绑定",exact:true}).click();
  };
  await openConfiguration();
  await expect(page.getByRole("dialog",{name:"账号详情"})).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog",{name:"账号详情"})).toHaveCount(0);
  await page.evaluate(async(mode)=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    let reads=0;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="listEndpointCredentialBindings"){
        reads+=1;
        if(reads===1)return mode==="transport"
          ?new Response(JSON.stringify({error:{code:"fixture_binding_read_failed",message:"synthetic binding read failed"}}),{status:503,headers:{"Content-Type":"application/json"}})
          :new Response(JSON.stringify([]),{status:200,headers:{"Content-Type":"application/json"}});
      }
      return original.call(this,operation,request);
    };
  },failedRead);
  await openConfiguration();
  const boundary=page.getByRole("dialog",{name:"未建立精确凭据映射"});
  await expect(boundary).toContainText(failedRead==="transport"?"synthetic binding read failed":"未将该凭据绑定到所选接口");
  await expect(page.getByRole("dialog",{name:"账号详情"})).toHaveCount(0);
  await boundary.getByRole("button",{name:"重新读取凭据配置"}).click();
  await expect(page.getByRole("dialog",{name:"账号详情"})).toBeVisible();
});

test("an admitted native status confirmation keeps its captured action after inventory changes", async ({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__nativeStatusBody",null);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:{body?:unknown}){
      if(operation==="listNativeAccounts")return new Response(JSON.stringify({items:[{id:"native-stable",provider:"grok_console",auth_status:"active",enabled:true,revision:7,import_batch_id:"native-stable",identity:{email:"native.stable@example.test",phone:null,username:null}}],next_cursor:null}),{status:200,headers:{"Content-Type":"application/json"}});
      if(operation==="updateNativeAccount"){
        Reflect.set(globalThis,"__nativeStatusBody",request.body);
        return new Response(JSON.stringify({error:{code:"management_native_account_conflict",message:"account changed"}}),{status:409,headers:{"Content-Type":"application/json"}});
      }
      const response=await original.call(this,operation,request);
      if(operation!=="listProviderAccountPools")return response;
      const value=await response.json();
      value.items=[{...value.items[0],provider_id:"grok-console-pool",channel_id:"ep-grok-console",account_id:"native-stable",account_kind:"grok_console_sso",presentation:{...value.items[0].presentation,provider:"Grok Console",category:"grok",identity:{email:"native.stable@example.test",phone:null,username:null}}}];
      return new Response(JSON.stringify(value),{status:response.status,headers:response.headers});
    };
  });
  await navigate(page,"账号池");
  await page.getByRole("navigation",{name:"账号视图"}).getByRole("button",{name:"运行状态",exact:true}).click();
  await page.locator(".account-desktop").getByRole("button",{name:"详情",exact:true}).click();
  const runtime=page.getByRole("dialog");
  await runtime.getByRole("button",{name:"配置与失败",exact:true}).click();
  await runtime.getByRole("button",{name:"查看凭据与绑定",exact:true}).click();
  const detail=page.getByRole("dialog",{name:"账号详情"});
  await detail.getByRole("button",{name:"配置",exact:true}).click();
  await detail.getByRole("button",{name:"停用",exact:true}).click();
  const confirm=page.getByRole("dialog",{name:"停用账号"});
  await page.evaluate(async()=>{
    const {queryClient}=await import("/src/api/queryClient.ts");
    queryClient.setQueriesData({queryKey:["runtime-native-resolution"]},(data:unknown)=>{
      if(!data||typeof data!=="object"||!("pages" in data))return data;
      const cached=data as {pages:{items:{enabled:boolean;revision:number}[]}[]};
      return {...cached,pages:cached.pages.map(page=>({...page,items:page.items.map(row=>({...row,enabled:false,revision:8}))}))};
    });
    await queryClient.invalidateQueries({queryKey:["accounts"]});
  });
  await expect(confirm).toBeVisible();
  await expect(confirm).toContainText("停用后");
  await confirm.getByRole("button",{name:"确认",exact:true}).click();
  await expect(confirm).toContainText("account changed");
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__nativeStatusBody"))).toEqual({revision:7,enabled:false});
});

test("an admitted native SSO editor survives a changed inventory while its write is held",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__nativeReplaceCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="listNativeAccounts")return new Response(JSON.stringify({items:[{id:"native-stable",provider:"grok_console",auth_status:"active",enabled:true,revision:7,import_batch_id:"native-stable",identity:{email:"native.stable@example.test",phone:null,username:null}}],next_cursor:null}),{status:200,headers:{"Content-Type":"application/json"}});
      if(operation==="replaceNativeAccountCredential"){
        Reflect.set(globalThis,"__nativeReplaceCalls",Number(Reflect.get(globalThis,"__nativeReplaceCalls"))+1);
        return new Promise<Response>(resolve=>{Reflect.set(globalThis,"__finishNativeReplace",()=>resolve(new Response(JSON.stringify({account_id:"native-stable",revision:8,removed:false,runtime_applied:false,identity_state:"observed"}),{status:200,headers:{"Content-Type":"application/json"}})));});
      }
      const response=await original.call(this,operation,request);
      if(operation!=="listProviderAccountPools")return response;
      const value=await response.json();
      value.items=[{...value.items[0],provider_id:"grok-console-pool",channel_id:"ep-grok-console",account_id:"native-stable",account_kind:"grok_console_sso",presentation:{...value.items[0].presentation,provider:"Grok Console",category:"grok",identity:{email:"native.stable@example.test",phone:null,username:null}}}];
      return new Response(JSON.stringify(value),{status:response.status,headers:response.headers});
    };
  });
  await navigate(page,"账号池");
  await page.getByRole("navigation",{name:"账号视图"}).getByRole("button",{name:"运行状态",exact:true}).click();
  await page.locator(".account-desktop").getByRole("button",{name:"详情",exact:true}).click();
  const runtime=page.getByRole("dialog");
  await runtime.getByRole("button",{name:"配置与失败",exact:true}).click();
  await runtime.getByRole("button",{name:"查看凭据与绑定",exact:true}).click();
  const detail=page.getByRole("dialog",{name:"账号详情"});
  await detail.getByRole("button",{name:"配置",exact:true}).click();
  await detail.getByRole("button",{name:"更新凭据",exact:true}).click();
  const editor=page.getByRole("dialog",{name:"更新 SSO 凭据"});
  await editor.getByRole("textbox",{name:"SSO 凭据"}).fill("synthetic-private-material");
  await page.evaluate(async()=>{
    const {queryClient}=await import("/src/api/queryClient.ts");
    queryClient.setQueriesData({queryKey:["runtime-native-resolution"]},(data:unknown)=>{
      if(!data||typeof data!=="object"||!("pages" in data))return data;
      const cached=data as {pages:{items:{enabled:boolean;revision:number}[]}[]};
      return {...cached,pages:cached.pages.map(page=>({...page,items:page.items.map(row=>({...row,enabled:false,revision:8}))}))};
    });
    await queryClient.invalidateQueries({queryKey:["accounts"]});
  });
  await expect(editor.getByRole("textbox",{name:"SSO 凭据"})).toHaveValue("synthetic-private-material");
  await editor.getByRole("button",{name:"保存并应用",exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>Number(Reflect.get(globalThis,"__nativeReplaceCalls")))).toBe(1);
  await page.evaluate(async()=>{const {queryClient}=await import("/src/api/queryClient.ts");await queryClient.invalidateQueries({queryKey:["accounts"]});});
  await expect(editor).toBeVisible();
  await expect(editor.getByRole("button",{name:"返回",exact:true})).toBeDisabled();
  await page.evaluate(()=>{const finish=Reflect.get(globalThis,"__finishNativeReplace") as (()=>void)|undefined;finish?.();});
  await expect(page.getByRole("dialog",{name:"账号修改已保存"})).toContainText("运行配置暂未应用");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__nativeReplaceCalls")))).toBe(1);
});
