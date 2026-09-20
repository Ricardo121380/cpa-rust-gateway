import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openAdvancedModels(page:import("@playwright/test").Page){
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"模型与路由");
}

async function rowMenu(page:import("@playwright/test").Page,modelName:string){
  const row=page.locator(".models-inventory tbody tr").filter({hasText:modelName}).first();
  const menu=row.locator(".row-menu");
  if(await menu.getAttribute("open")===null)await menu.locator("summary").click();
  return row;
}

test("model aliases use one guarded panel and preserve exact names",async({page})=>{
  await openAdvancedModels(page);
  const row=await rowMenu(page,"minimax-m3");
  await row.getByRole("button",{name:"管理别名"}).click();
  const browse=page.getByRole("dialog",{name:"模型别名 · minimax-m3"});
  await expect(browse).toBeVisible();
  await browse.getByRole("button",{name:"添加别名"}).click();
  const form=page.getByRole("dialog",{name:"添加模型别名"});
  await form.getByRole("textbox",{name:"客户端别名"}).fill("Exact.Alias-v2");
  await form.getByRole("button",{name:"取消"}).click();
  await expect(page.getByRole("alertdialog",{name:"放弃未保存的修改？"})).toBeVisible();
  await page.getByRole("alertdialog").getByRole("button",{name:"继续编辑"}).click();
  await expect(form.getByRole("textbox",{name:"客户端别名"})).toHaveValue("Exact.Alias-v2");
  await form.getByRole("button",{name:"创建"}).click();
  const receipt=page.getByRole("dialog",{name:"别名配置结果"});
  await expect(receipt).toContainText("已保存到当前草稿");
  await receipt.getByRole("button",{name:"完成"}).click();
  const rowAgain=await rowMenu(page,"minimax-m3");
  await rowAgain.getByRole("button",{name:"管理别名"}).click();
  const aliasPanel=page.getByRole("dialog",{name:"模型别名 · minimax-m3"});
  await expect(aliasPanel).toContainText("Exact.Alias-v2");
  await aliasPanel.getByRole("button",{name:"添加别名"}).click();
  const duplicate=page.getByRole("dialog",{name:"添加模型别名"});
  await duplicate.getByRole("textbox",{name:"客户端别名"}).fill("Exact.Alias-v2");
  await duplicate.getByRole("button",{name:"创建"}).click();
  await expect(duplicate.getByRole("alert")).toContainText("别名已存在");
  await duplicate.getByRole("button",{name:"取消"}).click();
  await page.getByRole("alertdialog").getByRole("button",{name:"放弃修改"}).click();
  const reopen=await rowMenu(page,"minimax-m3");
  await reopen.getByRole("button",{name:"管理别名"}).click();
  const aliasPanelAfter=page.getByRole("dialog",{name:"模型别名 · minimax-m3"});
  await expect(aliasPanelAfter).toContainText("Exact.Alias-v2");
  await aliasPanelAfter.getByRole("button",{name:"删除"}).click();
  const confirm=page.getByRole("dialog",{name:"删除模型别名"});
  await expect(confirm).toContainText("模型、路由、候选和授权保留");
  await confirm.getByRole("button",{name:"确认删除"}).click();
  const removed=page.getByRole("dialog",{name:"别名配置结果"});
  await expect(removed).toContainText("已保存到当前草稿");
  await removed.getByRole("button",{name:"完成"}).click();
  const finalRow=await rowMenu(page,"minimax-m3");
  await finalRow.getByRole("button",{name:"管理别名"}).click();
  await expect(page.getByRole("dialog",{name:"模型别名 · minimax-m3"})).not.toContainText("Exact.Alias-v2");
});

test("model editing accepts equivalent field order and freezes a held write",async({page})=>{
  await openAdvancedModels(page);
  const row=await rowMenu(page,"minimax-m3");
  await row.getByRole("button",{name:"编辑模型"}).click();
  const editor=page.getByRole("dialog",{name:"编辑 minimax-m3"});
  await editor.getByRole("textbox",{name:"管理端显示名"}).fill("Minimax 工作模型");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const response=await original.call(this,operation,request);
      if(operation!=="getPublicModel"||!response.ok)return response;
      const model=await response.clone().json() as {id:string;model_name:string;display_name:string;status:string;capabilities:Record<string,boolean>};
      const reordered={capabilities:Object.fromEntries(Object.entries(model.capabilities).reverse()),status:model.status,display_name:model.display_name,model_name:model.model_name,id:model.id};
      return new Response(JSON.stringify(reordered),{status:response.status,headers:response.headers});
    };
    holdFixtureOperationForTest("PATCH /admin/public-models/pm-minimax");
  });
  await editor.getByRole("button",{name:"保存"}).click();
  await expect.poll(()=>page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");return fixture.fixtureOperationCallsForTest("PATCH /admin/public-models/pm-minimax");})).toBe(1);
  await expect(editor.getByRole("textbox",{name:"管理端显示名"})).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(editor).toBeVisible();
  await page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");fixture.releaseFixtureOperationForTest("PATCH /admin/public-models/pm-minimax");});
  const receipt=page.getByRole("dialog",{name:"模型配置结果"});
  await expect(receipt).toContainText("已保存到当前草稿");
  await receipt.getByRole("button",{name:"完成"}).click();
  await expect(page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"}).first()).toContainText("Minimax 工作模型");
});

test("uncertain active-context alias retains its exact working draft through failed review",async({page})=>{
  await unlock(page);
  await page.evaluate(async()=>{
    const {call,callRevisioned}=await import("/src/api/client.ts");
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const versions=await call<{id:string;revision:string;status:string}[]>("listConfigVersions");
    const active=versions.find(row=>row.status==="active")!;
    const fork=await call<{id:string;revision:string}>("forkConfigVersion",{path:{config_version_id:active.id},headers:{"X-Config-Version":active.id,"If-Match":active.revision},body:{id:"qa-active-model",description:"准备模型维护"}});
    const created=await callRevisioned("createPublicModel",{headers:{"X-Config-Version":fork.id,"If-Match":fork.revision},body:{id:"qa-public-model",model_name:"Exact/QA-Model",display_name:"Exact/QA-Model",status:"active",capabilities:{streaming:true}}});
    await call("publishConfigVersion",{path:{config_version_id:fork.id},headers:{"If-Match":created.revision,"X-Expected-Active-Version":JSON.stringify(active.id),"X-Expected-Lifecycle-Event":"0"}});
    await call("createConfigVersion",{body:{id:"qa-other-draft",parent_id:fork.id,description:"添加模型别名"}});
    const selected=await call<import("/src/features/config-versions/versionStore.ts").ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:fork.id}});
    useVersionStore.getState().select(selected);
  });
  await navigate(page,"模型与路由");
  const row=await rowMenu(page,"Exact/QA-Model");
  await row.getByRole("button",{name:"管理别名"}).click();
  await page.getByRole("dialog",{name:"模型别名 · Exact/QA-Model"}).getByRole("button",{name:"添加别名"}).click();
  const form=page.getByRole("dialog",{name:"添加模型别名"});
  await form.getByRole("textbox",{name:"客户端别名"}).fill("Exact/QA-Alias");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__aliasReviewCounts",{fork:0,write:0});
    Reflect.set(globalThis,"__failExactReview",false);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const counts=Reflect.get(globalThis,"__aliasReviewCounts") as {fork:number;write:number};
      if(operation==="forkConfigVersion")counts.fork++;
      if(operation==="createModelAlias"){
        counts.write++;
        await original.call(this,operation,request);
        throw new Error("synthetic alias response lost");
      }
      if(operation==="getConfigVersion"&&Reflect.get(globalThis,"__failExactReview")){
        Reflect.set(globalThis,"__failExactReview",false);
        throw new Error("synthetic exact-version read failure");
      }
      return original.call(this,operation,request);
    };
  });
  await form.getByRole("button",{name:"创建"}).click();
  const receipt=page.getByRole("dialog",{name:"别名配置结果"});
  await expect(receipt).toContainText("结果未确认");
  await expect(receipt).toContainText("Exact/QA-Alias");
  const workingId=await receipt.locator("code.mono").innerText();
  expect(workingId).toMatch(/^edit-/u);
  await page.evaluate(()=>Reflect.set(globalThis,"__failExactReview",true));
  await receipt.getByRole("button",{name:"核对配置"}).click();
  await expect(receipt.getByRole("alert").filter({hasText:"重读失败"})).toContainText("重读失败");
  await expect(receipt).toContainText(workingId);
  await receipt.getByRole("button",{name:"核对配置"}).click();
  await expect(page).toHaveURL(/#\/versions/u);
  await expect(page.locator("main.canvas")).toHaveAttribute("data-context-version",workingId);
  expect(await page.evaluate(()=>Reflect.get(globalThis,"__aliasReviewCounts"))).toEqual({fork:1,write:1});
  const alias=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const page=await call<{items:{alias:string}[]}>("listModelAliases",{query:{limit:100}},{versionScoped:true});
    return page.items.find(row=>row.alias==="Exact/QA-Alias");
  });
  expect(alias?.alias).toBe("Exact/QA-Alias");
});

test("alias management reaches and removes a later-page exact alias",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    for(let index=0;index<=100;index++)await call("createModelAlias",{path:{public_model_id:"pm-minimax"},body:{alias:`QA.Page.${String(index).padStart(3,"0")}`}},{versionScoped:true,mutating:true});
  });
  const row=await rowMenu(page,"minimax-m3");
  await row.getByRole("button",{name:"管理别名"}).click();
  const dialog=page.getByRole("dialog",{name:"模型别名 · minimax-m3"});
  const later=dialog.locator(".data-toolbar",{hasText:"QA.Page.100"});
  await expect(later).toBeVisible();
  await later.getByRole("button",{name:"删除"}).click();
  await page.getByRole("dialog",{name:"删除模型别名"}).getByRole("button",{name:"确认删除"}).click();
  await page.getByRole("dialog",{name:"别名配置结果"}).getByRole("button",{name:"完成"}).click();
  const reopen=await rowMenu(page,"minimax-m3");
  await reopen.getByRole("button",{name:"管理别名"}).click();
  await expect(page.getByRole("dialog",{name:"模型别名 · minimax-m3"})).not.toContainText("QA.Page.100");
});

test("public-model deletion removes only its owned aliases, route, candidates and grants",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createModelAlias",{path:{public_model_id:"pm-minimax"},body:{alias:"Alias.To.Remove"}},{versionScoped:true,mutating:true});
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-minimax",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
    await call("createRouteCandidate",{path:{route_id:"rt-minimax"},body:{id:"cand-minimax",endpoint_id:"ep-relay-a-responses",upstream_model:"minimax-m3",credential_scope:"all_active",transform_mode:"passthrough",enabled:true,priority:0,weight:1,capability_override:{}}},{versionScoped:true,mutating:true});
  });
  const row=await rowMenu(page,"minimax-m3");
  await row.getByRole("button",{name:"删除模型"}).click();
  const confirm=page.getByRole("dialog",{name:"删除公开模型"});
  await expect(confirm).toContainText("访问组授权");
  await confirm.getByRole("button",{name:"确认删除"}).click();
  const receipt=page.getByRole("dialog",{name:"模型删除结果"});
  await expect(receipt).toContainText("已保存到当前草稿");
  await receipt.getByRole("button",{name:"完成"}).click();
  await expect(page.locator(".models-inventory tbody tr").filter({hasText:"minimax-m3"})).toHaveCount(0);
  const remaining=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const aliases=await call<{items:{alias:string}[]}>("listModelAliases",{query:{limit:100}},{versionScoped:true});
    const routes=await call<{items:{id:string}[]}>("listRoutes",{query:{limit:100}},{versionScoped:true});
    const candidates=await call<{items:{id:string}[]}>("listRouteCandidates",{query:{limit:100}},{versionScoped:true});
    const grants=await call<{route_id:string}[]>("listAccessGroupRoutes",{path:{access_group_id:"team-default"}},{versionScoped:true});
    return {alias:aliases.items.some(row=>row.alias==="Alias.To.Remove"),route:routes.items.some(row=>row.id==="rt-minimax"),candidate:candidates.items.some(row=>row.id==="cand-minimax"),grant:grants.some(row=>row.route_id==="rt-minimax")};
  });
  expect(remaining).toEqual({alias:false,route:false,candidate:false,grant:false});
});

test("a lost candidate-create response leaves a single non-replayable receipt",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-lost-candidate",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
  });
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const workbench=page.locator(".route-workbench");
  await workbench.getByRole("combobox",{name:"路由",exact:true}).selectOption("rt-lost-candidate");
  await workbench.getByRole("button",{name:"载入"}).click();
  await workbench.getByRole("button",{name:"加候选"}).click();
  const editor=page.getByRole("dialog",{name:"添加模型来源"});
  await editor.getByRole("textbox",{name:"候选标识"}).fill("cand-lost");
  await editor.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await editor.getByRole("textbox",{name:"上游原始模型 ID"}).fill("minimax-m3");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__lostCandidateWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="createRouteCandidate")return original.call(this,operation,request);
      Reflect.set(globalThis,"__lostCandidateWrites",Number(Reflect.get(globalThis,"__lostCandidateWrites"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic candidate response lost");
    };
  });
  await editor.getByRole("button",{name:"创建候选"}).click();
  const receipt=page.getByRole("dialog",{name:"候选配置结果"});
  await expect(receipt).toContainText("结果未确认");
  await expect(receipt.getByRole("button",{name:"创建候选"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__lostCandidateWrites")))).toBe(1);
  await receipt.getByRole("button",{name:"核对草稿"}).click();
  await expect(page).toHaveURL(/#\/versions/u);
});

for(const mode of ["lost","missing-etag"] as const){
  test(`uncertain route creation ${mode} reviews its exact draft without replay`,async({page})=>{
    await openAdvancedModels(page);
    const row=await rowMenu(page,"minimax-m3");
    await row.getByRole("button",{name:"配置路由"}).click();
    const editor=page.getByRole("dialog",{name:/创建路由/u});
    const routeId=`rt-${mode}`;
    await editor.getByRole("textbox",{name:"路由标识"}).fill(routeId);
    await page.evaluate(async(scenario)=>{
      const {ManagementApi}=await import("/src/generated/management-client.ts");
      const original=ManagementApi.prototype.request;
      Reflect.set(globalThis,"__routeReviewCounts",{write:0,fork:0,publish:0});
      Reflect.set(globalThis,"__failRouteReview",false);
      ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
        const counts=Reflect.get(globalThis,"__routeReviewCounts") as {write:number;fork:number;publish:number};
        if(operation==="forkConfigVersion")counts.fork++;
        if(operation==="publishConfigVersion")counts.publish++;
        if(operation==="getConfigVersion"&&Reflect.get(globalThis,"__failRouteReview")){
          Reflect.set(globalThis,"__failRouteReview",false);
          throw new Error("synthetic route review read failure");
        }
        if(operation!=="createRoute")return original.call(this,operation,request);
        counts.write++;
        const response=await original.call(this,operation,request);
        if(scenario==="lost")throw new Error("synthetic route response lost");
        const headers=new Headers(response.headers);
        headers.delete("ETag");
        return new Response(await response.clone().text(),{status:response.status,headers});
      };
    },mode);
    await editor.getByRole("button",{name:"创建路由"}).click();
    const receipt=page.getByRole("dialog",{name:"路由配置结果"});
    await expect(receipt).toContainText("结果未确认");
    await expect(receipt).toContainText(routeId);
    await expect(receipt).toContainText("draft-2026-08");
    await page.evaluate(()=>Reflect.set(globalThis,"__failRouteReview",true));
    await receipt.getByRole("button",{name:"核对草稿"}).click();
    await expect(receipt.getByRole("alert").filter({hasText:"重读失败"})).toBeVisible();
    await expect(receipt).toContainText(routeId);
    await receipt.getByRole("button",{name:"核对草稿"}).click();
    await expect(page).toHaveURL(/#\/versions/u);
    await expect(page.locator("main.canvas")).toHaveAttribute("data-context-version","draft-2026-08");
    expect(await page.evaluate(()=>Reflect.get(globalThis,"__routeReviewCounts"))).toEqual({write:1,fork:0,publish:0});
    const exists=await page.evaluate(async(id)=>{
      const {call}=await import("/src/api/client.ts");
      const routes=await call<{items:{id:string}[]}>("listRoutes",{query:{limit:100}},{versionScoped:true});
      return routes.items.some(row=>row.id===id);
    },routeId);
    expect(exists).toBe(true);
  });
}

test("editing candidate weight preserves arbitrary accepted override keys",async({page})=>{
  await openAdvancedModels(page);
  const override={"vision=false tools":true,"a=b":false,plain:false};
  await page.evaluate(async(data)=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-complex-override",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
    await call("createRouteCandidate",{path:{route_id:"rt-complex-override"},body:{id:"cand-complex-override",endpoint_id:"ep-relay-a-responses",upstream_model:"minimax-m3",credential_scope:"all_active",transform_mode:"passthrough",enabled:true,priority:0,weight:1,capability_override:data}},{versionScoped:true,mutating:true});
  },override);
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const workbench=page.locator(".route-workbench");
  await workbench.getByRole("combobox",{name:"路由",exact:true}).selectOption("rt-complex-override");
  await workbench.getByRole("button",{name:"载入"}).click();
  const inventory=page.getByRole("region",{name:"完整配置资源"});
  await inventory.getByRole("button",{name:"候选",exact:true}).click();
  await inventory.getByRole("button",{name:"编辑候选"}).click();
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await expect(editor.getByRole("textbox",{name:"能力覆盖（可留空）"})).toHaveValue(/"vision=false tools": true/u);
  await editor.getByRole("spinbutton",{name:"权重"}).fill("5");
  await editor.getByRole("button",{name:"保存候选"}).click();
  await page.getByRole("dialog",{name:"候选配置结果"}).getByRole("button",{name:"完成"}).click();
  const saved=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const page=await call<{items:{id:string;weight:number;capability_override:Record<string,boolean>}[]}>("listRouteCandidates",{query:{limit:100}},{versionScoped:true});
    return page.items.find(row=>row.id==="cand-complex-override");
  });
  expect(saved).toEqual(expect.objectContaining({weight:5,capability_override:override}));
});

test("a changed draft revision rejects a stale candidate editor before another write",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-stale",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
    await call("createRouteCandidate",{path:{route_id:"rt-stale"},body:{id:"cand-stale",endpoint_id:"ep-relay-a-responses",upstream_model:"minimax-m3",credential_scope:"all_active",transform_mode:"passthrough",enabled:true,priority:0,weight:1,capability_override:{}}},{versionScoped:true,mutating:true});
  });
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const inventory=page.getByRole("region",{name:"完整配置资源"});
  await inventory.getByRole("button",{name:"候选",exact:true}).click();
  const row=inventory.locator("tr",{hasText:"cand-stale"});
  await row.getByRole("button",{name:"编辑候选"}).click();
  const editor=page.getByRole("dialog",{name:"编辑模型来源"});
  await editor.getByRole("spinbutton",{name:"权重"}).fill("2");
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    await call("updateRouteCandidate",{path:{route_id:"rt-stale",candidate_id:"cand-stale"},body:{id:"cand-stale",endpoint_id:"ep-relay-a-responses",upstream_model:"minimax-m3",credential_scope:"all_active",transform_mode:"passthrough",enabled:true,priority:0,weight:3,capability_override:{}}},{versionScoped:true,mutating:true});
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__staleEditorWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="updateRouteCandidate")Reflect.set(globalThis,"__staleEditorWrites",Number(Reflect.get(globalThis,"__staleEditorWrites"))+1);
      return original.call(this,operation,request);
    };
  });
  await editor.getByRole("button",{name:"保存候选"}).click();
  await expect(editor.getByRole("alert")).toContainText("草稿已变化");
  await expect(editor.getByRole("spinbutton",{name:"权重"})).toBeEnabled();
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__staleEditorWrites")))).toBe(0);
});

test("a delayed validation for route A never labels route B",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createPublicModel",{body:{id:"pm-second-route",model_name:"Second/Model",display_name:"Second/Model",status:"active",capabilities:{streaming:true}}},{versionScoped:true,mutating:true});
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-a",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
    await call("createRoute",{path:{public_model_id:"pm-second-route"},body:{id:"rt-b",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
  });
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const workbench=page.locator(".route-workbench");
  const picker=workbench.getByRole("combobox",{name:"路由",exact:true});
  await picker.selectOption("rt-a");
  await workbench.getByRole("button",{name:"载入"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const response=await original.call(this,operation,request);
      if(operation==="validateRoute")await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseRouteAValidation",resolve));
      return response;
    };
  });
  await workbench.getByRole("button",{name:"校验"}).click();
  await expect.poll(()=>page.evaluate(()=>typeof Reflect.get(globalThis,"__releaseRouteAValidation"))).toBe("function");
  await picker.selectOption("rt-b");
  await workbench.getByRole("button",{name:"载入"}).click();
  await page.evaluate(()=>{(Reflect.get(globalThis,"__releaseRouteAValidation") as ()=>void)();});
  await expect(workbench.locator(".rw-validation")).toHaveCount(0);
});

test("a revision change during route validation leaves no stale success",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-validation-race",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
  });
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const workbench=page.locator(".route-workbench");
  await workbench.getByRole("combobox",{name:"路由",exact:true}).selectOption("rt-validation-race");
  await workbench.getByRole("button",{name:"载入"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      const response=await original.call(this,operation,request);
      if(operation==="validateRoute")await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseRevisionValidation",resolve));
      return response;
    };
  });
  await workbench.getByRole("button",{name:"校验"}).click();
  await expect.poll(()=>page.evaluate(()=>typeof Reflect.get(globalThis,"__releaseRevisionValidation"))).toBe("function");
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("updateRoute",{path:{route_id:"rt-validation-race"},body:{id:"rt-validation-race",policy:"smooth_weighted_round_robin",max_attempts:4,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
    (Reflect.get(globalThis,"__releaseRevisionValidation") as ()=>void)();
  });
  await expect(workbench.locator(".rw-validation")).toHaveCount(0);
  await expect(workbench.getByRole("alert")).toContainText("草稿已变化");
});

test("a held route edit freezes fields and missing revision receipt never replays",async({page})=>{
  await openAdvancedModels(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("createRoute",{path:{public_model_id:"pm-minimax"},body:{id:"rt-held-edit",policy:"smooth_weighted_round_robin",max_attempts:3,bootstrap_timeout_ms:30000}},{versionScoped:true,mutating:true});
  });
  await page.getByText("高级路由、候选与别名",{exact:true}).click();
  const workbench=page.locator(".route-workbench");
  await workbench.getByRole("combobox",{name:"路由",exact:true}).selectOption("rt-held-edit");
  await workbench.getByRole("button",{name:"载入"}).click();
  await workbench.getByRole("button",{name:"编辑路由"}).click();
  const editor=page.getByRole("dialog",{name:"编辑路由"});
  await editor.getByRole("spinbutton",{name:"最大尝试次数"}).fill("5");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__heldRouteWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="updateRoute")return original.call(this,operation,request);
      Reflect.set(globalThis,"__heldRouteWrites",Number(Reflect.get(globalThis,"__heldRouteWrites"))+1);
      const response=await original.call(this,operation,request);
      const headers=new Headers(response.headers);headers.delete("ETag");
      return new Response(await response.text(),{status:response.status,headers});
    };
    holdFixtureOperationForTest("PATCH /admin/routes/rt-held-edit");
  });
  await editor.getByRole("button",{name:"保存路由"}).click();
  await expect.poll(()=>page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");return fixture.fixtureOperationCallsForTest("PATCH /admin/routes/rt-held-edit");})).toBe(1);
  await expect(editor.getByRole("spinbutton",{name:"最大尝试次数"})).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(editor).toBeVisible();
  await page.evaluate(async()=>{const fixture=await import("/src/dev/fixtures.ts");fixture.releaseFixtureOperationForTest("PATCH /admin/routes/rt-held-edit");});
  const receipt=page.getByRole("dialog",{name:"路由配置结果"});
  await expect(receipt).toContainText("结果未确认");
  await expect(receipt.getByRole("button",{name:"保存路由"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__heldRouteWrites")))).toBe(1);
});
