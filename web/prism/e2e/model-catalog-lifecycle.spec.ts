import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openExistingModel(page:import("@playwright/test").Page){
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"模型与路由");
  const row=page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"});
  await expect(row).toBeVisible();
  await row.getByRole("button",{name:"管理连接"}).click();
}

async function connectExistingSource(page:import("@playwright/test").Page){
  await openExistingModel(page);
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"添加来源连接"}).click();
  const form=page.getByRole("dialog",{name:"接入模型"});
  await form.getByRole("textbox",{name:"上游模型 ID"}).fill("Exact/Model-v2");
  await form.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await form.getByRole("button",{name:"保存并应用"}).click();
  await page.getByRole("dialog",{name:"模型接入结果"}).getByRole("button",{name:"核对配置"}).click();
  await page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"}).getByRole("button",{name:"管理连接"}).click();
}

test("an exact upstream ID connects to the selected public model and remains editable",async({page})=>{
  await openExistingModel(page);
  const inspector=page.getByRole("dialog",{name:"minimax-m3 · 来源连接"});
  await expect(inspector).toContainText("尚未连接提供商");
  await inspector.getByRole("button",{name:"添加来源连接"}).click();
  const form=page.getByRole("dialog",{name:"接入模型"});
  await form.getByRole("textbox",{name:"上游模型 ID"}).fill("Exact/Model-v2");
  await form.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await form.getByRole("button",{name:"保存并应用"}).click();
  const receipt=page.getByRole("dialog",{name:"模型接入结果"});
  await expect(receipt).toContainText("已保存到当前草稿");
  await receipt.getByRole("button",{name:"核对配置"}).click();
  const row=page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"});
  await row.getByRole("button",{name:"管理连接"}).click();
  const connected=page.getByRole("dialog",{name:"minimax-m3 · 来源连接"});
  await expect(connected).toContainText("Exact/Model-v2");
  await connected.getByRole("button",{name:"编辑路径"}).click();
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await expect(editor).toContainText("Exact/Model-v2");
  await editor.getByRole("spinbutton",{name:"权重"}).fill("7");
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存到当前草稿");
});

test("an unchanged active connection does not create an unused configuration fork",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"模型与路由");
  await page.getByRole("button",{name:"接入模型",exact:true}).click();
  const first=page.getByRole("dialog",{name:"接入模型"});
  await first.getByRole("textbox",{name:"上游模型 ID"}).fill("gpt-5.6-terra");
  await first.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await first.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"模型接入结果"})).toContainText("已保存到当前草稿");
  await page.getByRole("dialog",{name:"模型接入结果"}).getByRole("button",{name:"核对配置"}).click();
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  await page.getByRole("region",{name:"待应用变更",exact:true}).getByRole("button",{name:"校验并应用",exact:true}).click();
  await page.getByRole("dialog",{name:"确认应用配置"}).getByRole("button",{name:"确认应用",exact:true}).click();
  await expect(page.getByRole("dialog")).toContainText("已确认");
  await page.getByRole("button",{name:"完成",exact:true}).click();
  await navigate(page,"模型与路由");
  const row=page.locator(".models-inventory tbody tr").filter({hasText:"gpt-5.6-terra"});
  await row.getByRole("button",{name:"管理连接"}).click();
  await page.getByRole("dialog",{name:"gpt-5.6-terra · 来源连接"}).getByRole("button",{name:"添加来源连接"}).click();
  const second=page.getByRole("dialog",{name:"接入模型"});
  await expect(second.getByRole("textbox",{name:"上游模型 ID"})).toHaveValue("gpt-5.6-terra");
  await second.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__noopForks",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="forkConfigVersion")Reflect.set(globalThis,"__noopForks",Number(Reflect.get(globalThis,"__noopForks"))+1);
      return original.call(this,operation,request);
    };
  });
  await second.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"模型接入结果"})).toContainText("本次没有修改配置");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__noopForks")))).toBe(0);
});

test("a rejected candidate after route creation keeps a non-replayable partial receipt",async({page})=>{
  await openExistingModel(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__routeCreates",0);
    Reflect.set(globalThis,"__candidateCreates",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="createRoute")Reflect.set(globalThis,"__routeCreates",Number(Reflect.get(globalThis,"__routeCreates"))+1);
      if(operation==="createRouteCandidate"){
        Reflect.set(globalThis,"__candidateCreates",Number(Reflect.get(globalThis,"__candidateCreates"))+1);
        return new Response(JSON.stringify({error:{code:"fixture_candidate_rejected",message:"synthetic candidate rejection"}}),{status:400,headers:{"Content-Type":"application/json"}});
      }
      return original.call(this,operation,request);
    };
  });
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"添加来源连接"}).click();
  const form=page.getByRole("dialog",{name:"接入模型"});
  await form.getByRole("textbox",{name:"上游模型 ID"}).fill("Exact/Model-v2");
  await form.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await form.getByRole("button",{name:"保存并应用"}).click();
  const receipt=page.getByRole("dialog",{name:"模型接入结果"});
  await expect(receipt).toContainText("已有 1 步保存");
  await expect(receipt).toContainText("synthetic candidate rejection");
  await expect(receipt.getByRole("button",{name:"保存并应用"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__routeCreates")))).toBe(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__candidateCreates")))).toBe(1);
});

test("removing the last enabled source explains and applies the model disable",async({page})=>{
  await connectExistingSource(page);
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"移除路径"}).click();
  const confirm=page.getByRole("dialog",{name:"移除模型来源"});
  await expect(confirm).toContainText("Exact/Model-v2");
  await expect(confirm).toContainText("最后一个启用来源，公开模型也会停用");
  await confirm.getByRole("button",{name:"确认移除"}).click();
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存到当前草稿");
  await page.getByRole("dialog",{name:"模型来源结果"}).getByRole("button",{name:"核对配置"}).click();
  await expect(page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"})).toContainText("已停用");
});

test("a failed candidate delete after disabling the last source retains a partial receipt",async({page})=>{
  await connectExistingSource(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__modelDisableWrites",0);
    Reflect.set(globalThis,"__sourceDeleteWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="updatePublicModel")Reflect.set(globalThis,"__modelDisableWrites",Number(Reflect.get(globalThis,"__modelDisableWrites"))+1);
      if(operation==="deleteRouteCandidate"){
        Reflect.set(globalThis,"__sourceDeleteWrites",Number(Reflect.get(globalThis,"__sourceDeleteWrites"))+1);
        return new Response(JSON.stringify({error:{code:"fixture_delete_rejected",message:"synthetic delete rejection"}}),{status:400,headers:{"Content-Type":"application/json"}});
      }
      return original.call(this,operation,request);
    };
  });
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"移除路径"}).click();
  await page.getByRole("dialog",{name:"移除模型来源"}).getByRole("button",{name:"确认移除"}).click();
  const receipt=page.getByRole("dialog",{name:"模型来源结果"});
  await expect(receipt).toContainText("已有 1 步保存");
  await expect(receipt).toContainText("synthetic delete rejection");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__modelDisableWrites")))).toBe(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__sourceDeleteWrites")))).toBe(1);
  await expect(receipt.getByRole("button",{name:"确认移除"})).toHaveCount(0);
});

test("editing or removing an already-disabled source does not silently disable the public model",async({page})=>{
  await openExistingModel(page);
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"添加来源连接"}).click();
  const form=page.getByRole("dialog",{name:"接入模型"});
  await form.getByRole("textbox",{name:"上游模型 ID"}).fill("Exact/Model-v2");
  await form.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await form.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"模型接入结果"})).toBeVisible();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__unexpectedModelDisables",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="updatePublicModel")Reflect.set(globalThis,"__unexpectedModelDisables",Number(Reflect.get(globalThis,"__unexpectedModelDisables"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="listRouteCandidates"||!response.ok)return response;
      const body=await response.json();
      return new Response(JSON.stringify({...body,items:body.items.map((candidate:{id:string;enabled:boolean})=>({...candidate,enabled:false}))}),{status:response.status,headers:response.headers});
    };
  });
  await page.getByRole("dialog",{name:"模型接入结果"}).getByRole("button",{name:"核对配置"}).click();
  const row=page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"});
  await row.getByRole("button",{name:"管理连接"}).click();
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"编辑路径"}).click();
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await expect(editor).not.toContainText("最后一个启用来源");
  await editor.getByRole("spinbutton",{name:"权重"}).fill("4");
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存到当前草稿");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__unexpectedModelDisables")))).toBe(0);
  await page.getByRole("dialog",{name:"模型来源结果"}).getByRole("button",{name:"核对配置"}).click();
  await row.getByRole("button",{name:"管理连接"}).click();
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"移除路径"}).click();
  const confirm=page.getByRole("dialog",{name:"移除模型来源"});
  await expect(confirm).not.toContainText("公开模型也会停用");
  await confirm.getByRole("button",{name:"确认移除"}).click();
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存到当前草稿");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__unexpectedModelDisables")))).toBe(0);
});

test("a changed sibling source blocks a captured edit before any model write",async({page})=>{
  await connectExistingSource(page);
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"编辑路径"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__siblingConflictWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(["updatePublicModel","updateRouteCandidate"].includes(operation))Reflect.set(globalThis,"__siblingConflictWrites",Number(Reflect.get(globalThis,"__siblingConflictWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="listRouteCandidates"||!response.ok)return response;
      const body=await response.json();
      return new Response(JSON.stringify({...body,items:[...body.items,{...body.items[0],id:"new-sibling",enabled:true}]}),{status:response.status,headers:response.headers});
    };
  });
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await editor.getByRole("spinbutton",{name:"权重"}).fill("3");
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect(editor.getByRole("alert")).toContainText("其他来源连接已变化");
  await expect(editor.getByRole("checkbox",{name:"启用此来源"})).toBeEnabled();
  await expect(editor.getByRole("spinbutton",{name:"优先级"})).toBeEnabled();
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toBeEnabled();
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__siblingConflictWrites")))).toBe(0);
});

test("a held source update freezes every field until its captured write finishes",async({page})=>{
  await connectExistingSource(page);
  const route=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const response=await call<{items:readonly {id:string;route_id:string}[]}>("listRouteCandidates",{query:{limit:100}},{versionScoped:true});
    const candidate=response.items[0];
    if(!candidate)throw new Error("missing synthetic candidate");
    return `PATCH /admin/routes/${candidate.route_id}/candidates/${candidate.id}`;
  });
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"编辑路径"}).click();
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await editor.getByRole("spinbutton",{name:"权重"}).fill("2");
  await page.evaluate(async path=>{
    const fixture=await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(path);
  },route);
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect.poll(()=>page.evaluate(async path=>{
    const fixture=await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(path);
  },route)).toBe(1);
  await expect(editor.getByRole("checkbox",{name:"启用此来源"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"优先级"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toHaveValue("2");
  await page.keyboard.press("Escape");
  await expect(editor).toBeVisible();
  await page.evaluate(async path=>{
    const fixture=await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(path);
  },route);
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存到当前草稿");
  await page.getByRole("dialog",{name:"模型来源结果"}).getByRole("button",{name:"核对配置"}).click();
  await page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"}).getByRole("button",{name:"管理连接"}).click();
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"编辑路径"}).click();
  await expect(page.getByRole("dialog",{name:"编辑模型来源"}).getByRole("spinbutton",{name:"权重"})).toHaveValue("2");
});

test("source fields stay frozen while a saved edit waits for application",async({page})=>{
  await connectExistingSource(page);
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"关闭",exact:true}).click();
  await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
  await page.getByRole("region",{name:"待应用变更",exact:true}).getByRole("button",{name:"校验并应用",exact:true}).click();
  await page.getByRole("dialog",{name:"确认应用配置"}).getByRole("button",{name:"确认应用",exact:true}).click();
  await expect(page.getByRole("dialog")).toContainText("已确认");
  await page.getByRole("button",{name:"完成",exact:true}).click();
  await navigate(page,"模型与路由");
  const row=page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"});
  await row.getByRole("button",{name:"管理连接"}).click();
  await page.getByRole("dialog",{name:"minimax-m3 · 来源连接"}).getByRole("button",{name:"编辑路径"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__heldPublishCalls",0);
    Reflect.set(globalThis,"__heldCandidateUpdates",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="updateRouteCandidate")Reflect.set(globalThis,"__heldCandidateUpdates",Number(Reflect.get(globalThis,"__heldCandidateUpdates"))+1);
      if(operation==="publishConfigVersion"){
        Reflect.set(globalThis,"__heldPublishCalls",Number(Reflect.get(globalThis,"__heldPublishCalls"))+1);
        await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseModelPublish",resolve));
      }
      return original.call(this,operation,request);
    };
  });
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await editor.getByRole("spinbutton",{name:"权重"}).fill("5");
  await editor.getByRole("button",{name:"保存并应用"}).click();
  await expect.poll(()=>page.evaluate(()=>Number(Reflect.get(globalThis,"__heldPublishCalls")))).toBe(1);
  await expect(editor.getByRole("checkbox",{name:"启用此来源"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"优先级"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toBeDisabled();
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toHaveValue("5");
  await page.keyboard.press("Escape");
  await expect(editor).toBeVisible();
  await page.evaluate(()=>{const release=Reflect.get(globalThis,"__releaseModelPublish") as (()=>void)|undefined;release?.();});
  await expect(page.getByRole("dialog",{name:"模型来源结果"})).toContainText("已保存并应用");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__heldCandidateUpdates")))).toBe(1);
});

async function chooseCatalogModel(page:import("@playwright/test").Page){
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"模型目录");
  const browser=page.getByRole("region",{name:"上游模型清单"});
  await browser.getByRole("combobox",{name:"接口",exact:true}).selectOption("ep-relay-a-responses");
  await browser.getByRole("combobox",{name:"目录账号",exact:true}).selectOption("cred-relay-key");
  await expect(browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"})).toBeVisible();
  await browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"}).check();
  await browser.getByRole("button",{name:"接入所选模型（1）"}).click();
  return page.getByRole("dialog",{name:"确认接入上游模型"});
}

test("catalog confirmation preserves exact ID and keeps account evidence separate from routing",async({page})=>{
  const confirm=await chooseCatalogModel(page);
  await expect(confirm).toContainText("gpt-5.6-terra");
  await expect(confirm).toContainText("运行调度仍按已配置的可用账号池");
  await confirm.getByRole("button",{name:"确认接入"}).click();
  const result=page.getByRole("dialog",{name:"模型接入结果"});
  await expect(result).toContainText("gpt-5.6-terra");
  await expect(result).toContainText("已保存到当前草稿");
  await result.getByRole("button",{name:"核对配置"}).click();
  await expect(page.locator(".models-inventory")).toContainText("gpt-5.6-terra");
});

test("a changed catalog snapshot blocks connection before any model write",async({page})=>{
  const confirm=await chooseCatalogModel(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__catalogModelWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="createPublicModel"||operation==="createRoute"||operation==="createRouteCandidate")Reflect.set(globalThis,"__catalogModelWrites",Number(Reflect.get(globalThis,"__catalogModelWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="listCatalogModels")return response;
      const data=await response.json();
      return new Response(JSON.stringify({...data,target:{...data.target,snapshot_version:data.target.snapshot_version+1}}),{status:response.status,headers:response.headers});
    };
  });
  await confirm.getByRole("button",{name:"确认接入"}).click();
  await expect(confirm).toContainText("目录已更新或过期");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__catalogModelWrites")))).toBe(0);
  await confirm.getByRole("button",{name:"取消"}).click();
  await expect(page.getByRole("checkbox",{name:"选择 gpt-5.6-terra"})).toBeChecked();
});

test("catalog confirmation rejects a newer admitted draft before model writes",async({page})=>{
  const confirm=await chooseCatalogModel(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__staleCatalogWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(["createPublicModel","createRoute","createRouteCandidate"].includes(operation))Reflect.set(globalThis,"__staleCatalogWrites",Number(Reflect.get(globalThis,"__staleCatalogWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="listConfigVersions"||!response.ok)return response;
      const versions=await response.json();
      return new Response(JSON.stringify(versions.map((version:{id:string;revision:string})=>version.id==="draft-2026-08"?{...version,revision:"rev-999"}:version)),{status:response.status,headers:response.headers});
    };
  });
  await confirm.getByRole("button",{name:"确认接入"}).click();
  await expect(confirm.getByRole("alert")).toContainText("草稿已变化");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__staleCatalogWrites")))).toBe(0);
});

test("catalog confirmation rejects a changed working endpoint before model writes",async({page})=>{
  const confirm=await chooseCatalogModel(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__changedEndpointWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(["createPublicModel","createRoute","createRouteCandidate"].includes(operation))Reflect.set(globalThis,"__changedEndpointWrites",Number(Reflect.get(globalThis,"__changedEndpointWrites"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="getEndpoint"||!response.ok)return response;
      const endpoint=await response.json();
      return new Response(JSON.stringify({...endpoint,base_url:"https://changed.example.test/v1"}),{status:response.status,headers:response.headers});
    };
  });
  await confirm.getByRole("button",{name:"确认接入"}).click();
  await expect(confirm.getByRole("alert")).toContainText("所选接口已变化");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__changedEndpointWrites")))).toBe(0);
});

test("catalog continuation retains an earlier selection and connects a page-two exact ID",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    let firstPage:Record<string,unknown>|undefined;
    Reflect.set(globalThis,"__catalogCursorReads",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="listCatalogModels")return original.call(this,operation,request);
      const query=(request as {query?:{cursor?:string}}).query;
      if(query?.cursor==="synthetic-page-2"){
        Reflect.set(globalThis,"__catalogCursorReads",Number(Reflect.get(globalThis,"__catalogCursorReads"))+1);
        return new Response(JSON.stringify({...firstPage,items:[{model:"gpt-5.6-terra",present_in_last_success:true}],next_cursor:null}),{status:200,headers:{"Content-Type":"application/json"}});
      }
      const response=await original.call(this,operation,request);
      if(!response.ok)return response;
      firstPage=await response.json();
      return new Response(JSON.stringify({...firstPage,items:Array.from({length:100},(_,index)=>({model:`PageOne-${String(index).padStart(3,"0")}`,present_in_last_success:true})),total_count:101,next_cursor:"synthetic-page-2"}),{status:response.status,headers:response.headers});
    };
  });
  await navigate(page,"模型目录");
  const browser=page.getByRole("region",{name:"上游模型清单"});
  await browser.getByRole("combobox",{name:"接口",exact:true}).selectOption("ep-relay-a-responses");
  await browser.getByRole("combobox",{name:"目录账号",exact:true}).selectOption("cred-relay-key");
  await browser.getByRole("checkbox",{name:"选择 PageOne-000"}).check();
  await browser.getByRole("button",{name:"加载更多模型"}).click();
  await expect(browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"})).toBeVisible();
  await expect(browser.getByRole("checkbox",{name:"选择 PageOne-000"})).toBeChecked();
  for(let index=1;index<20;index++)await browser.getByRole("checkbox",{name:`选择 PageOne-${String(index).padStart(3,"0")}`}).check();
  await expect(browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"})).toBeDisabled();
  for(let index=1;index<20;index++)await browser.getByRole("checkbox",{name:`选择 PageOne-${String(index).padStart(3,"0")}`}).uncheck();
  await browser.getByRole("checkbox",{name:"选择 PageOne-000"}).uncheck();
  await browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"}).check();
  await browser.getByRole("button",{name:"接入所选模型（1）"}).click();
  await page.getByRole("dialog",{name:"确认接入上游模型"}).getByRole("button",{name:"确认接入"}).click();
  await expect(page.getByRole("dialog",{name:"模型接入结果"})).toContainText("gpt-5.6-terra");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__catalogCursorReads")))).toBeGreaterThan(0);
});

test("a rejected catalog cursor stops selection until an explicit reread",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="listCatalogModels")return original.call(this,operation,request);
      if((request as {query?:{cursor?:string}}).query?.cursor)return new Response(JSON.stringify({error:{code:"cursor_revision_conflict",message:"catalog cursor changed"}}),{status:409,headers:{"Content-Type":"application/json"}});
      const response=await original.call(this,operation,request);
      if(!response.ok)return response;
      const body=await response.json();
      return new Response(JSON.stringify({...body,next_cursor:"stale-cursor"}),{status:response.status,headers:response.headers});
    };
  });
  await navigate(page,"模型目录");
  const browser=page.getByRole("region",{name:"上游模型清单"});
  await browser.getByRole("combobox",{name:"接口",exact:true}).selectOption("ep-relay-a-responses");
  await browser.getByRole("combobox",{name:"目录账号",exact:true}).selectOption("cred-relay-key");
  await browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"}).check();
  await browser.getByRole("button",{name:"加载更多模型"}).click();
  await expect(browser.getByRole("alert")).toContainText("后续目录页读取失败");
  await expect(browser.getByRole("button",{name:"接入所选模型（1）"})).toBeDisabled();
  await browser.getByRole("button",{name:"重读目录"}).click();
  await expect(browser.getByRole("alert")).toHaveCount(0);
  await expect(browser.getByRole("checkbox",{name:"选择 gpt-5.6-terra"})).not.toBeChecked();
});
