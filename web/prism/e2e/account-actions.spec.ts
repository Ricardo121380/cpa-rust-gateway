import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock, clearVersionForTest } from "./helpers";

async function inspect(page: Page, account: string) {
  await page.getByRole("textbox", { name: "搜索已加载账号" }).fill(account);
  const details = page.locator(".account-desktop").getByRole("button", { name: "详情", exact: true });
  await expect(details).toHaveCount(1);
  await details.click();
}

async function loseRuntimeActionResponse(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__runtimeActionCalls", 0);
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation !== "applyProviderAccountPoolAction") return original.call(this, operation, request);
      Reflect.set(globalThis, "__runtimeActionCalls", Number(Reflect.get(globalThis, "__runtimeActionCalls")) + 1);
      await original.call(this, operation, request);
      throw new Error("simulated lost runtime action response");
    };
  });
}

async function loseCredentialUpdateResponse(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__credentialUpdateCalls", 0);
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation !== "updateCredential") return original.call(this, operation, request);
      Reflect.set(globalThis, "__credentialUpdateCalls", Number(Reflect.get(globalThis, "__credentialUpdateCalls")) + 1);
      await original.call(this, operation, request);
      throw new Error("simulated lost credential update response");
    };
  });
}

async function staleNextCredentialRead(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__credentialStatusWrites", 0);
    let stale = true;
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation === "updateCredentialStatus") Reflect.set(globalThis, "__credentialStatusWrites", Number(Reflect.get(globalThis, "__credentialStatusWrites")) + 1);
      const response = await original.call(this, operation, request);
      if (operation !== "getCredential" || !stale) return response;
      stale = false;
      const body = await response.json() as { revision: number };
      return new Response(JSON.stringify({ ...body, revision: body.revision + 1 }), { status: response.status, headers: response.headers });
    };
  });
}

async function activateDraft(page: Page): Promise<void> {
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", {name:"查看变更",exact:true}).click();
  await page.getByRole("region",{name:"待应用变更",exact:true}).getByRole("button",{name:"校验并应用",exact:true}).click();
  await page.getByRole("dialog", {name:"确认应用配置"}).getByRole("button", {name:"确认应用",exact:true}).click();
  await page.getByRole("dialog").getByRole("button", {name:"完成",exact:true}).click();
}

async function failCredentialPublication(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__credentialUpdateCalls", 0);
    Reflect.set(globalThis, "__credentialPublicationCalls", 0);
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation === "updateCredential") Reflect.set(globalThis, "__credentialUpdateCalls", Number(Reflect.get(globalThis, "__credentialUpdateCalls")) + 1);
      if (operation !== "publishConfigVersion") return original.call(this, operation, request);
      Reflect.set(globalThis, "__credentialPublicationCalls", Number(Reflect.get(globalThis, "__credentialPublicationCalls")) + 1);
      throw new Error("simulated credential publication transport failure");
    };
  });
}

test("accounts read without a version, while cooldown requires a selected version and confirmation", async ({ page }) => {
  await unlock(page);
  await clearVersionForTest(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-relay-key");
  await expect(page.getByRole("dialog").getByRole("button", { name: "冷却账号" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await selectDraft(page);
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-relay-key");
  await page.getByRole("dialog").getByRole("button", { name: "冷却账号" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("runtime.member@example.test");
  await expect(confirm.locator(".identity-details")).not.toHaveAttribute("open", "");
  await confirm.getByLabel("冷却时长", { exact: false }).fill("500");
  await confirm.getByRole("button", { name: "确认冷却" }).click();
  await expect(confirm).toBeVisible();
  await confirm.getByLabel("冷却时长", { exact: false }).fill("60000");
  await confirm.getByRole("button", { name: "确认冷却" }).click();
  await expect(page.locator(".action-notice")).toContainText("冷却");
});

test("recovery and target conflicts remain runtime facts", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-grok-oauth");
  await page.getByRole("dialog").getByRole("button", { name: "请求恢复", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认请求恢复" }).click();
  await expect(page.locator(".action-notice")).toContainText("需要人工恢复");
  await inspect(page, "cred-grok-old");
  await page.getByRole("dialog").getByRole("button", { name: "冷却账号" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认冷却" }).click();
  await expect(page.getByRole("alert")).toContainText("目标快照已改变");
  await expect(page.locator(".conflict-bar")).toHaveCount(0);
});

test("lost runtime action response retains the exact connection without replay", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await loseRuntimeActionResponse(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-relay-key");
  await page.getByRole("dialog").getByRole("button", { name: "冷却账号" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认冷却" }).click();
  await expect(page.locator(".action-notice")).toContainText("结果未确认");
  await expect(page.locator(".action-notice")).toContainText("runtime.member@example.test");
  await expect(page.locator(".action-notice")).toContainText("Responses · api.example.test");
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__runtimeActionCalls")))).toBe(1);
  await expect(page.getByRole("button", {name: "确认冷却"})).toHaveCount(0);
});

test("lost credential update response becomes a review-only receipt", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await loseCredentialUpdateResponse(page);
  await navigate(page, "账号池");
  await page.getByRole("textbox", { name: "搜索账号", exact: true }).fill("alex@example.test");
  const row=page.locator(".account-list tbody tr");
  await expect(row).toHaveCount(1);
  await row.getByRole("button", {name:"更多",exact:true}).click();
  await page.getByRole("dialog", {name:"账号操作"}).getByRole("button", {name:"更新凭据",exact:true}).click();
  const update=page.getByRole("dialog", {name:"更新账号凭据"});
  await update.getByRole("textbox", {name:"新的 API Key 或完整授权文件"}).fill("replacement-material");
  await update.getByRole("button", {name:"保存并应用",exact:true}).click();
  await expect(update).toContainText("凭据更新结果未确认");
  await expect(update.getByRole("button", {name:"保存并应用",exact:true})).toHaveCount(0);
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__credentialUpdateCalls")))).toBe(1);
});

test("a no-op batch verifies the captured credential before declaring it unchanged", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", {name:"批量管理",exact:true}).click();
  await page.locator(".account-list").getByRole("checkbox").first().check();
  await staleNextCredentialRead(page);
  await page.getByRole("button", {name:"启用",exact:true}).click();
  const dialog=page.getByRole("dialog", {name:"启用 1 份授权"});
  await dialog.getByRole("button", {name:"确认启用",exact:true}).click();
  await expect(dialog).toContainText("账号授权已变化");
  await expect(dialog).toContainText("未执行：被拒绝");
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__credentialStatusWrites")))).toBe(0);
});

test("runtime configuration details resolve an opaque ordinary credential by owner and binding", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("navigation", {name: "账号视图"}).getByRole("button", {name: "运行状态", exact: true}).click();
  await inspect(page, "cred-relay-key");
  const runtime=page.getByRole("dialog");
  await runtime.getByRole("button", {name:"配置与失败",exact:true}).click();
  await runtime.getByRole("button", {name:"查看凭据与绑定",exact:true}).click();
  const credential=page.getByRole("dialog", {name:"账号详情"});
  await expect(credential).toContainText("runtime.member@example.test");
  await expect(credential).toContainText("API");
});

test("credential replacement retains its saved result when publication fails", async ({ page }) => {
  await unlock(page);
  await activateDraft(page);
  await failCredentialPublication(page);
  await navigate(page, "账号池");
  await page.getByRole("textbox", { name: "搜索账号", exact: true }).fill("alex@example.test");
  const row=page.locator(".account-list tbody tr");
  await expect(row).toHaveCount(1);
  await row.getByRole("button", {name:"更多",exact:true}).click();
  await page.getByRole("dialog", {name:"账号操作"}).getByRole("button", {name:"更新凭据",exact:true}).click();
  const update=page.getByRole("dialog", {name:"更新账号凭据"});
  await update.getByRole("textbox", {name:"新的 API Key 或完整授权文件"}).fill("replacement-material");
  await update.getByRole("button", {name:"保存并应用",exact:true}).click();
  await expect(update).toContainText("凭据已保存，但应用尚未完成");
  await expect(update.getByRole("button", {name:"保存并应用",exact:true})).toHaveCount(0);
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__credentialUpdateCalls")))).toBe(1);
  expect(await page.evaluate(() => Number(Reflect.get(globalThis, "__credentialPublicationCalls")))).toBe(1);
});

for (const changedRead of [1, 2]) test(`batch removal rejects a recreated ID under another provider on read ${changedRead}`, async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("textbox", {name:"搜索账号",exact:true}).fill("alex@example.test");
  await expect(page.locator(".account-list tbody tr")).toHaveCount(1);
  await page.locator(".account-list tbody tr").getByRole("button", {name:"更多",exact:true}).click();
  await page.getByRole("dialog", {name:"账号操作"}).getByRole("button", {name:"移除授权",exact:true}).click();
  await page.evaluate(async (changedRead) => {
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    let reads=0;
    Reflect.set(globalThis,"__recreatedWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="deleteCredential"||operation==="updateCredentialStatus")Reflect.set(globalThis,"__recreatedWrites",Number(Reflect.get(globalThis,"__recreatedWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="getCredential")return response;
      reads+=1;
      if(reads!==changedRead)return response;
      const value=await response.json();
      return new Response(JSON.stringify({...value,upstream_id:"other-provider"}),{status:response.status,headers:response.headers});
    };
  },changedRead);
  const confirm=page.getByRole("dialog", {name:"移除 1 份授权"});
  await confirm.getByRole("button", {name:"确认移除",exact:true}).click();
  await expect(confirm).toContainText("账号授权已变化");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__recreatedWrites")))).toBe(0);
  const retained=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const credential=await call<{upstream_id:string}>("getCredential",{path:{credential_id:"cred-codex-oauth"}},{versionScoped:true});
    const bindings=await call<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:"ep-relay-a-responses"}},{versionScoped:true});
    return {owner:credential.upstream_id,bound:bindings.some(row=>row.credential_id==="cred-codex-oauth")};
  });
  expect(retained).toEqual({owner:"relay-a",bound:true});
});

test("credential replacement rejects a recreated ID before sending material", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"账号池");
  await page.getByRole("textbox",{name:"搜索账号",exact:true}).fill("alex@example.test");
  await expect(page.locator(".account-list tbody tr")).toHaveCount(1);
  await page.locator(".account-list tbody tr").getByRole("button",{name:"更多",exact:true}).click();
  await page.getByRole("dialog",{name:"账号操作"}).getByRole("button",{name:"更新凭据",exact:true}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__replacementWrites",0);
    let reads=0;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="updateCredential")Reflect.set(globalThis,"__replacementWrites",Number(Reflect.get(globalThis,"__replacementWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="getCredential")return response;
      reads+=1;
      if(reads!==1)return response;
      const value=await response.json();
      return new Response(JSON.stringify({...value,upstream_id:"other-provider"}),{status:response.status,headers:response.headers});
    };
  });
  const update=page.getByRole("dialog",{name:"更新账号凭据"});
  await update.getByRole("textbox",{name:"新的 API Key 或完整授权文件"}).fill("replacement-material");
  await update.getByRole("button",{name:"保存并应用",exact:true}).click();
  await expect(update).toContainText("账号授权已变化");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__replacementWrites")))).toBe(0);
  const retained=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const credential=await call<{upstream_id:string}>("getCredential",{path:{credential_id:"cred-codex-oauth"}},{versionScoped:true});
    const bindings=await call<readonly {credential_id:string}[]>("listEndpointCredentialBindings",{path:{endpoint_id:"ep-relay-a-responses"}},{versionScoped:true});
    return {owner:credential.upstream_id,bound:bindings.some(row=>row.credential_id==="cred-codex-oauth")};
  });
  expect(retained).toEqual({owner:"relay-a",bound:true});
});

for(const [observedStatus,choice] of [["unauthorized","active"],["cooling","disabled"]] as const) test(`credential replacement handles read-only ${observedStatus} with an explicit status choice`,async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async(status)=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__replacementStatuses",[]);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:{body?:{status?:string}}){
      if(operation==="updateCredential"){
        (Reflect.get(globalThis,"__replacementStatuses") as string[]).push(request.body?.status??"missing");
        return original.call(this,operation,request);
      }
      const response=await original.call(this,operation,request);
      if(operation!=="listAccountInventory"&&operation!=="getCredential")return response;
      const value=await response.json();
      if(operation==="getCredential")return new Response(JSON.stringify({...value,status}),{status:response.status,headers:response.headers});
      value.items=value.items.map((row:{id:string;managed?:{credential?:{status:string}}})=>row.id==="cred-codex-oauth"?{...row,managed:{...row.managed,credential:{...row.managed?.credential,status}}}:row);
      return new Response(JSON.stringify(value),{status:response.status,headers:response.headers});
    };
  },observedStatus);
  await navigate(page,"账号池");
  await page.getByRole("textbox",{name:"搜索账号",exact:true}).fill("alex@example.test");
  await expect(page.locator(".account-list tbody tr")).toHaveCount(1);
  await page.locator(".account-list tbody tr").getByRole("button",{name:"更多",exact:true}).click();
  await page.getByRole("dialog",{name:"账号操作"}).getByRole("button",{name:"更新凭据",exact:true}).click();
  const update=page.getByRole("dialog",{name:"更新账号凭据"});
  await expect(update.getByRole("combobox",{name:"替换后的账号状态"})).toBeVisible();
  await update.getByRole("textbox",{name:"新的 API Key 或完整授权文件"}).fill("replacement-material");
  await update.getByRole("button",{name:"保存并应用",exact:true}).click();
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__replacementStatuses"))).toEqual([]);
  await update.getByRole("combobox",{name:"替换后的账号状态"}).selectOption(choice);
  await update.getByRole("button",{name:"保存并应用",exact:true}).click();
  await expect(update).toContainText("凭据已保存");
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__replacementStatuses"))).toEqual([choice]);
});
