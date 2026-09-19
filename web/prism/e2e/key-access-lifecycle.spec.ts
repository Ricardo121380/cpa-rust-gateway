import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("editing one key creates a private permission group and retains its sibling",async({page})=>{
  await unlock(page);
  await navigate(page,"访问控制");
  const original=page.locator(".key-list tbody tr").filter({hasText:"rgw_9f3c21ab04d7e6b2"});
  const sibling=page.locator(".key-list tbody tr").filter({hasText:"rgw_00dead00deadbeef"});
  await original.getByRole("button",{name:"编辑"}).click();
  const editor=page.getByRole("dialog",{name:"编辑 API 密钥"});
  await expect(editor.getByRole("textbox",{name:"名称"})).toHaveValue("默认组");
  await editor.getByRole("textbox",{name:"名称"}).fill("Dedicated client");
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"密钥权限结果"})).toContainText("已保存并应用");
  await page.getByRole("dialog",{name:"密钥权限结果"}).getByRole("button",{name:"完成"}).click();
  await expect(original).toContainText("Dedicated client");
  await expect(sibling).toContainText("默认组");
});

test("a rejected key move retains the saved group as a non-replayable partial result",async({page})=>{
  await unlock(page);
  await navigate(page,"访问控制");
  await page.locator(".key-list tbody tr").filter({hasText:"rgw_9f3c21ab04d7e6b2"}).getByRole("button",{name:"编辑"}).click();
  const editor=page.getByRole("dialog",{name:"编辑 API 密钥"});
  await editor.getByRole("textbox",{name:"名称"}).fill("Partial client");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__keyGroupCreates",0);
    Reflect.set(globalThis,"__keyMoveAttempts",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="createAccessGroup")Reflect.set(globalThis,"__keyGroupCreates",Number(Reflect.get(globalThis,"__keyGroupCreates"))+1);
      if(operation==="updateClientKey"){
        Reflect.set(globalThis,"__keyMoveAttempts",Number(Reflect.get(globalThis,"__keyMoveAttempts"))+1);
        return new Response(JSON.stringify({error:{code:"fixture_key_move_rejected",message:"synthetic key move rejection"}}),{status:400,headers:{"Content-Type":"application/json"}});
      }
      return original.call(this,operation,request);
    };
  });
  await editor.getByRole("button",{name:"保存并应用"}).click();
  const receipt=page.getByRole("dialog",{name:"密钥权限结果"});
  await expect(receipt).toContainText("已有 1 步保存");
  await expect(receipt).toContainText("synthetic key move rejection");
  await expect(receipt.getByRole("button",{name:"保存并应用"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__keyGroupCreates")))).toBe(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__keyMoveAttempts")))).toBe(1);
});

test("button-only model selection participates in the key form discard guard",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.getByRole("button",{name:"创建客户端密钥"}).click();
  const dialog=page.getByRole("dialog",{name:"创建 API 密钥"});
  await dialog.getByRole("button",{name:"全选当前已开放模型"}).click();
  await expect(dialog.getByRole("checkbox",{name:"minimax-m3"})).toBeChecked();
  await dialog.getByRole("button",{name:"取消"}).click();
  await expect(page.getByRole("alertdialog",{name:"放弃未保存的修改？"})).toBeVisible();
  await page.getByRole("alertdialog").getByRole("button",{name:"继续编辑"}).click();
  await expect(dialog.getByRole("checkbox",{name:"minimax-m3"})).toBeChecked();
  await dialog.getByRole("button",{name:"清空选择"}).click();
  await dialog.getByRole("button",{name:"取消"}).click();
  await expect(dialog).toHaveCount(0);
});

test("a lost issuance response does not reveal or reissue a persisted key",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-key-loss",policy:"smooth_weighted_round_robin",max_attempts:1,bootstrap_timeout_ms:1000}},{versionScoped:true,mutating:true});
  });
  await navigate(page,"访问控制");
  await page.getByRole("button",{name:"创建客户端密钥"}).click();
  const dialog=page.getByRole("dialog",{name:"创建 API 密钥"});
  await dialog.getByRole("textbox",{name:"名称"}).fill("Lost receipt client");
  await dialog.getByRole("checkbox",{name:"minimax-m3"}).check();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__lostIssueCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="issueClientKey")return original.call(this,operation,request);
      Reflect.set(globalThis,"__lostIssueCalls",Number(Reflect.get(globalThis,"__lostIssueCalls"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic issuance response lost");
    };
  });
  await dialog.getByRole("button",{name:"创建并应用"}).click();
  const receipt=page.getByRole("dialog",{name:"密钥签发结果"});
  await expect(receipt).toContainText("密钥记录已存在，但完整密钥无法再次读取");
  await expect(receipt.locator(".reveal-key")).toHaveCount(0);
  await expect(receipt.getByRole("button",{name:"创建并应用"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__lostIssueCalls")))).toBe(1);
});

test("a route-missing preflight keeps the key form editable without creating a group",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.getByRole("button",{name:"创建客户端密钥"}).click();
  const dialog=page.getByRole("dialog",{name:"创建 API 密钥"});
  await dialog.getByRole("textbox",{name:"名称"}).fill("Needs route");
  await dialog.getByRole("checkbox",{name:"minimax-m3"}).check();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__routeMissingGroupWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="createAccessGroup")Reflect.set(globalThis,"__routeMissingGroupWrites",Number(Reflect.get(globalThis,"__routeMissingGroupWrites"))+1);
      return original.call(this,operation,request);
    };
  });
  await dialog.getByRole("button",{name:"创建并应用"}).click();
  await expect(dialog.getByRole("alert")).toContainText("尚未配置接口连接");
  await expect(dialog.getByRole("textbox",{name:"名称"})).toBeEnabled();
  await expect(dialog.getByRole("button",{name:"创建并应用"})).toBeEnabled();
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__routeMissingGroupWrites")))).toBe(0);
});

test("advanced group signing requires a choice and guards its one-time reveal",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button",{name:"按访问组签发"}).click();
  const dialog=page.getByRole("dialog",{name:"按访问组签发"});
  await expect(dialog.getByRole("combobox",{name:"访问组"})).toHaveValue("");
  await dialog.getByRole("textbox",{name:"Key ID"}).fill("advanced-key");
  await expect(dialog.getByRole("button",{name:"签发到草稿"})).toBeDisabled();
  await dialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");fixture.holdFixtureOperationForTest("POST /admin/client-keys");});
  await dialog.getByRole("button",{name:"签发到草稿"}).click();
  await expect.poll(()=>page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");return fixture.fixtureOperationCallsForTest("POST /admin/client-keys");})).toBe(1);
  await expect(dialog.getByRole("textbox",{name:"Key ID"})).toBeDisabled();
  await expect(dialog.getByRole("combobox",{name:"访问组"})).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");fixture.releaseFixtureOperationForTest("POST /admin/client-keys");});
  const reveal=page.getByRole("dialog",{name:"Client Key 已签发"});
  await expect(reveal.locator(".reveal-key")).toBeVisible();
  await expect(reveal).toContainText("已保存到当前草稿");
  await reveal.getByRole("button",{name:"完成并清除密钥"}).click();
  await expect(page.locator(".reveal-key")).toHaveCount(0);
  await expect(page.locator(".key-list tbody tr")).toHaveCount(3);
});

test("advanced signing does not recover or reveal after draft ownership changes",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button",{name:"按访问组签发"}).click();
  const dialog=page.getByRole("dialog",{name:"按访问组签发"});
  await dialog.getByRole("textbox",{name:"Key ID"}).fill("abandoned-signing");
  await dialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__oldDraftId",useVersionStore.getState().context?.configVersionId);
    Reflect.set(globalThis,"__oldDraftRecoveryReads",0);
    Reflect.set(globalThis,"__advancedIssueCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="issueClientKey")Reflect.set(globalThis,"__advancedIssueCalls",Number(Reflect.get(globalThis,"__advancedIssueCalls"))+1);
      if(operation==="listClientKeys"&&JSON.stringify(request).includes(String(Reflect.get(globalThis,"__oldDraftId"))))Reflect.set(globalThis,"__oldDraftRecoveryReads",Number(Reflect.get(globalThis,"__oldDraftRecoveryReads"))+1);
      return original.call(this,operation,request);
    };
    holdFixtureOperationForTest("POST /admin/client-keys");
  });
  await dialog.getByRole("button",{name:"签发到草稿"}).click();
  await expect.poll(()=>page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");return fixture.fixtureOperationCallsForTest("POST /admin/client-keys");})).toBe(1);
  await page.evaluate(async()=>{
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const {releaseFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    useVersionStore.getState().select({id:"replacement-draft",status:"draft",revision:"rev-1",created_at_ms:0,description:"replacement"});
    releaseFixtureOperationForTest("POST /admin/client-keys");
  });
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".reveal-key")).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__advancedIssueCalls")))).toBe(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__oldDraftRecoveryReads")))).toBe(0);
});

test("advanced signing keeps a genuine lost response as a redacted review result",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button",{name:"按访问组签发"}).click();
  const dialog=page.getByRole("dialog",{name:"按访问组签发"});
  await dialog.getByRole("textbox",{name:"Key ID"}).fill("advanced-lost-response");
  await dialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__advancedLostCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="issueClientKey")return original.call(this,operation,request);
      Reflect.set(globalThis,"__advancedLostCalls",Number(Reflect.get(globalThis,"__advancedLostCalls"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic advanced issuance response lost");
    };
  });
  await dialog.getByRole("button",{name:"签发到草稿"}).click();
  const receipt=page.getByRole("dialog",{name:"签发结果"});
  await expect(receipt).toContainText("密钥记录已存在，但完整密钥无法再次读取");
  await expect(receipt.locator(".reveal-key")).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__advancedLostCalls")))).toBe(1);
});

test("advanced signing drops a held response after the session is replaced",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button",{name:"按访问组签发"}).click();
  const dialog=page.getByRole("dialog",{name:"按访问组签发"});
  await dialog.getByRole("textbox",{name:"Key ID"}).fill("replaced-session-signing");
  await dialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__replacementOldDraft",useVersionStore.getState().context?.configVersionId);
    Reflect.set(globalThis,"__replacementRecoveryReads",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="listClientKeys"&&JSON.stringify(request).includes(String(Reflect.get(globalThis,"__replacementOldDraft"))))Reflect.set(globalThis,"__replacementRecoveryReads",Number(Reflect.get(globalThis,"__replacementRecoveryReads"))+1);
      return original.call(this,operation,request);
    };
    holdFixtureOperationForTest("POST /admin/client-keys");
  });
  await dialog.getByRole("button",{name:"签发到草稿"}).click();
  await expect.poll(()=>page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");return fixture.fixtureOperationCallsForTest("POST /admin/client-keys");})).toBe(1);
  await page.evaluate(async()=>{
    const {useSessionStore}=await import("/src/session/sessionStore.ts");
    const {releaseFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const state=useSessionStore.getState();
    if(!state.managementKey)throw new Error("missing fixture session");
    state.unlock(state.managementKey,state.csrfToken);
    releaseFixtureOperationForTest("POST /admin/client-keys");
  });
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".reveal-key")).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__replacementRecoveryReads")))).toBe(0);
});

test("advanced signing abandons reconciliation if ownership changes during its GET",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.locator("details",{hasText:"高级访问组"}).getByText("高级访问组",{exact:true}).click();
  await page.getByRole("button",{name:"按访问组签发"}).click();
  const dialog=page.getByRole("dialog",{name:"按访问组签发"});
  await dialog.getByRole("textbox",{name:"Key ID"}).fill("recovery-owner-change");
  await dialog.getByRole("combobox",{name:"访问组"}).selectOption("team-default");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__recoveryOldDraft",useVersionStore.getState().context?.configVersionId);
    Reflect.set(globalThis,"__recoveryGetCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="issueClientKey"){
        await original.call(this,operation,request);
        throw new Error("synthetic advanced response loss");
      }
      if(operation==="listClientKeys"&&JSON.stringify(request).includes(String(Reflect.get(globalThis,"__recoveryOldDraft")))){
        Reflect.set(globalThis,"__recoveryGetCalls",Number(Reflect.get(globalThis,"__recoveryGetCalls"))+1);
        const response=await original.call(this,operation,request);
        await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseRecoveryGet",resolve));
        return response;
      }
      return original.call(this,operation,request);
    };
  });
  await dialog.getByRole("button",{name:"签发到草稿"}).click();
  await expect.poll(()=>page.evaluate(()=>typeof Reflect.get(globalThis,"__releaseRecoveryGet"))).toBe("function");
  await page.evaluate(async()=>{
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    useVersionStore.getState().select({id:"replacement-recovery-draft",status:"draft",revision:"rev-1",created_at_ms:0,description:"replacement"});
    (Reflect.get(globalThis,"__releaseRecoveryGet") as ()=>void)();
  });
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".reveal-key")).toHaveCount(0);
  await expect(page.getByRole("dialog",{name:"签发结果"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__recoveryGetCalls")))).toBe(1);
});

test("revocation keeps the captured key and a reviewable application receipt",async({page})=>{
  await unlock(page);
  await navigate(page,"访问控制");
  const row=page.locator(".key-list tbody tr").filter({hasText:"rgw_9f3c21ab04d7e6b2"});
  await row.getByRole("button",{name:"吊销"}).click();
  const confirm=page.getByRole("dialog",{name:"吊销 API 密钥"});
  await expect(confirm).toContainText("rgw_9f3c21ab04d7e6b2");
  await confirm.getByRole("button",{name:"确认吊销"}).click();
  const receipt=page.getByRole("dialog",{name:"密钥吊销结果"});
  await expect(receipt).toContainText("已保存并应用");
  await receipt.getByRole("button",{name:"完成"}).click();
  await expect(row).toContainText("已吊销");
});
