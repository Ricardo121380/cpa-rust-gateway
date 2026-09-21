import { resourceChoices } from "./resource-choice-fixtures";
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock, selectVersion } from "./helpers";

// The route half of the configuration chain.
//
// Before this, ModelsPage could CREATE a route and nothing else. A route with
// no candidate is rejected by the backend
// (management_mutation_service.rs:2074 route_missing_active_candidate), so
// every route made in the panel left the draft in a state the panel could not
// repair — the operator had to roll back or reach for curl.
//
// The loop below is the acceptance criterion for that fix, and it only means
// something because the fixture reproduces the failure: it returns
// route_missing_active_candidate for a candidate-less route rather than
// answering `valid: true` and hiding the dead end.

async function openModels(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "模型与路由");
  await page.getByText("高级路由、候选与别名", {exact:true}).click();
  await expect(page.locator(".route-workbench")).toBeVisible();
}

async function makeRoute(
  page: import("@playwright/test").Page,
  routeId: string,
): Promise<void> {
  await page.getByRole("button", { name: "高级模型配置" }).click();
  const model = page.getByRole("dialog");
  await model.getByLabel("模型 ID").fill(`pm-${routeId}`);
  await model.getByLabel("模型名", { exact: false }).fill(`m-${routeId}`);
  await model.getByRole("button", { name: "保存" }).click();
  const modelReceipt=page.getByRole("dialog",{name:"模型配置结果"});
  await expect(modelReceipt).toContainText("已保存到当前草稿");
  await modelReceipt.getByRole("button",{name:"完成"}).click();

  const row=page.locator(".models-inventory tbody tr", { hasText: `m-${routeId}` }).first();
  await row.locator(".row-menu summary").click();
  await row.getByRole("button", { name: "配置路由" }).click();
  const routeSheet = page.getByRole("dialog");
  await routeSheet.getByLabel("路由标识").fill(routeId);
  await routeSheet.getByRole("button", { name: "创建路由" }).click();
  const routeReceipt=page.getByRole("dialog",{name:"路由配置结果"});
  await expect(routeReceipt).toContainText("已保存到当前草稿");
  await expect(routeReceipt).toContainText("路由已创建");
  await routeReceipt.getByRole("button",{name:"继续配置候选"}).click();
  await expect(page.locator("details.models-advanced")).toHaveAttribute("open","");
}

test("a new route fails validation until a candidate is added", async ({ page }) => {
  await openModels(page);
  await makeRoute(page, "rt-e2e");
  const inventory = page.getByRole("region", { name: "完整配置资源" });
  await expect(inventory.locator('[data-resource-id="rt-e2e"]').first()).toBeVisible();
  await expect(inventory.locator('[data-resource-id="pm-rt-e2e"]').first()).toBeVisible();


  // The panel says what it just did to the draft rather than reporting success.
  await expect(page.locator(".action-notice").first()).toContainText("添加候选后再校验");

  await page.locator(".route-workbench").getByRole("button", { name: "打开它" }).click();
  await expect(page.locator(".route-workbench")).toContainText("smooth_weighted_round_robin");

  await page.locator(".route-workbench").getByRole("button", { name: "校验" }).click();
  const validation = page.locator(".rw-validation");
  await expect(validation).toHaveAttribute("data-valid", "false");
  await expect(validation).toContainText("route_missing_active_candidate");
  // The raw code AND a translation: an operator greps the first, reads the
  // second.
  await expect(validation).toContainText("路由没有任何启用的候选");
  // validate is a draft-topology check only; publish admission happens later.
  await expect(validation).toContainText("这里通过不等于发布会通过");

  await page.locator(".route-workbench").getByRole("button", { name: "加候选" }).click();
  const sheet = page.getByRole("region",{name:"添加模型来源",exact:true});
  await expect(sheet).toContainText("上游原始模型 ID");
  await sheet.getByLabel("候选标识").fill("cand-e2e");
  await sheet.getByRole("combobox", { name: "接口连接", exact: true }).selectOption("ep-relay-a-responses");
  await sheet.getByLabel("上游原始模型 ID", { exact: true }).fill("relay-x");
  await sheet.getByRole("button", { name: "创建候选" }).click();
  const candidateReceipt=page.locator('.inline-workspace, [role="dialog"]').filter({has:page.getByRole("heading",{name:"候选配置结果",exact:true})});
  await expect(candidateReceipt).toContainText("已保存到当前草稿");
  await candidateReceipt.getByRole("button",{name:"完成"}).click();

  // Adding a candidate re-validates on its own — the operator should not have
  // to re-ask whether the thing they just fixed is fixed.
  await expect(page.locator(".rw-validation")).toHaveAttribute("data-valid", "true");
  await inventory.getByRole("button", { name: "候选", exact: true }).click();
  await expect(inventory.locator('[data-resource-id="cand-e2e"]').first()).toBeVisible();
  await expect(inventory).toContainText("relay-x");
  await inventory.getByRole("button", { name: "编辑候选" }).click();
  const editor = page.getByRole("region",{name:"编辑模型来源",exact:true});
  await expect(editor.getByRole("textbox",{name:"候选标识"})).toHaveValue("cand-e2e");
  await expect(editor.getByRole("textbox",{name:"候选标识"})).toHaveAttribute("readonly","");
  await editor.getByLabel("权重").fill("7");
  await editor.getByLabel("优先级").fill("3");
  await editor.getByLabel("协议转换").selectOption("canonical_bridge");
  await editor.getByLabel("能力覆盖").fill("vision=false tools=true");
  await editor.getByRole("button", { name: "保存候选" }).click();
  const editReceipt=page.locator('.inline-workspace, [role="dialog"]').filter({has:page.getByRole("heading",{name:"候选配置结果",exact:true})});
  await expect(editReceipt).toContainText("已保存到当前草稿");
  await editReceipt.getByRole("button",{name:"完成"}).click();
  await expect(inventory).toContainText("权重 7");
  await inventory.getByRole("button", { name: "编辑候选" }).click();
  await expect(editor.getByLabel("优先级")).toHaveValue("3");
  await expect(editor.getByLabel("协议转换")).toHaveValue("canonical_bridge");
  await expect(editor.getByLabel("能力覆盖")).toHaveValue("vision=false tools=true");
  await editor.getByRole("button", { name: "取消", exact: true }).click();
  await inventory.getByRole("button", { name: "删除候选" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认删除" }).click();
  await page.locator('.inline-workspace, [role="dialog"]').filter({has:page.getByRole("heading",{name:"候选配置结果",exact:true})}).getByRole("button",{name:"完成"}).click();
  await expect(inventory.locator('[data-resource-id="cand-e2e"]')).toHaveCount(0);
  await expect(page.locator(".rw-validation")).toHaveAttribute("data-valid", "false");
  await inventory.getByRole("button", { name: "路由", exact: true }).click();
  await expect(inventory.locator('[data-resource-id="rt-e2e"]').first()).toBeVisible();

  await inventory.getByRole("button", { name: "别名", exact: true }).click();
  await expect(inventory).toContainText("此版本暂无该类资源");

  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    await call("grantAccessGroupRoute",{path:{access_group_id:"team-default"},body:{route_id:"rt-e2e",enabled:true}},{versionScoped:true,mutating:true});
  });

  const workbench=page.locator(".route-workbench");
  await workbench.getByRole("button",{name:"编辑路由"}).click();
  const routeEditor=page.getByRole("dialog",{name:"编辑路由"});
  await routeEditor.getByRole("spinbutton",{name:"最大尝试次数"}).fill("4");
  await routeEditor.getByRole("button",{name:"保存路由"}).click();
  const routeEditReceipt=page.getByRole("dialog",{name:"路由配置结果"});
  await expect(routeEditReceipt).toContainText("已保存到当前草稿");
  await routeEditReceipt.getByRole("button",{name:"完成"}).click();
  await expect(workbench).toContainText("4");
  await workbench.getByRole("button",{name:"删除路由"}).click();
  const routeDelete=page.getByRole("dialog",{name:"删除路由"});
  await expect(routeDelete).toContainText("访问组授权");
  await routeDelete.getByRole("button",{name:"确认删除"}).click();
  const routeDeleteReceipt=page.getByRole("dialog",{name:"路由配置结果"});
  await expect(routeDeleteReceipt).toContainText("已保存到当前草稿");
  await routeDeleteReceipt.getByRole("button",{name:"完成"}).click();
  await inventory.getByRole("button",{name:"路由",exact:true}).click();
  await expect(inventory.locator('[data-resource-id="rt-e2e"]')).toHaveCount(0);
  const grants=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    return await call<{route_id:string}[]>("listAccessGroupRoutes",{path:{access_group_id:"team-default"}},{versionScoped:true});
  });
  expect(grants.some(grant=>grant.route_id==="rt-e2e")).toBe(false);

});

test("capability_override rejects a non-boolean instead of coercing it", async ({ page }) => {
  await openModels(page);
  await makeRoute(page, "rt-e2e-cap");
  await page.locator(".route-workbench").getByRole("button", { name: "打开它" }).click();
  await page.locator(".route-workbench").getByRole("button", { name: "加候选" }).click();

  const sheet = page.getByRole("region",{name:"添加模型来源",exact:true});
  await sheet.getByLabel("候选标识").fill("cand-cap");
  await sheet.getByRole("combobox", { name: "接口连接", exact: true }).selectOption("ep-relay-a-responses");
  await sheet.getByLabel("上游原始模型 ID", { exact: true }).fill("relay-x");
  await sheet.getByLabel("能力覆盖").fill("vision=1");
  await sheet.getByRole("button", { name: "创建候选" }).click();

  await expect(sheet.getByRole("alert")).toContainText("只能是 true 或 false");
});

async function selectActive(page: import("@playwright/test").Page): Promise<void> {
  await selectVersion(page, "v-2026-07");
}

test("explain on a draft says the snapshot is missing, not that the panel is unwired", async ({
  page,
}) => {
  // Measured against a real gateway: explain_route resolves against a compiled
  // snapshot and a draft has none, so it 503s exactly like an unwired
  // projection. Blaming the deployment there sends the operator looking for a
  // problem that is not theirs.
  await unlock(page);
  await selectDraft(page);
  await resourceChoices(page,{routes:["rt-minimax","rt-multi-provider"],upstreams:["prov-a"]});
  await navigate(page, "运行诊断");
  await page.getByRole("form", { name: "路由解释" }).getByRole("combobox", { name: "route_id" }).selectOption("rt-minimax");
  await page.getByLabel("请求模型").fill("minimax-m3");
  await page.getByRole("button", { name: "解释" }).click();

  const explainCard = page.locator(".rt-card", { hasText: "路由解释" });
  await expect(explainCard).toContainText("草稿版本没有可解释的快照");
  await expect(explainCard).toContainText("快照只在版本发布后存在");
});

test("route explain shows price evidence and the catalog it came from", async ({ page }) => {
  await unlock(page);
  await selectActive(page);
  await resourceChoices(page,{routes:["rt-minimax","rt-multi-provider"],upstreams:["prov-a"]});
  await navigate(page, "运行诊断");

  await page.getByRole("form", { name: "路由解释" }).getByRole("combobox", { name: "route_id" }).selectOption("rt-minimax");
  await page.getByLabel("请求模型").fill("minimax-m3");
  await page.getByRole("button", { name: "解释" }).click();

  await expect(page.locator(".rt-price-policy")).toContainText("cat-2026-08");
  await expect(page.locator(".rt-price-policy")).toContainText("rate_dominance_v1");
  // Rates, not request cost — the distinction the backend is careful about.
  await expect(page.locator(".rt-price-policy")).toContainText("不是本次请求的花费");
  await expect(page.locator('.rt-chip[data-state="dominant"]')).toBeVisible();
  await expect(page.locator('.rt-chip[data-state="unpriced"]')).toBeVisible();
});

test("a multi-Provider route asks for a Provider instead of failing generically", async ({
  page,
}) => {
  await unlock(page);
  await selectActive(page);
  await resourceChoices(page,{routes:["rt-minimax","rt-multi-provider"],upstreams:["prov-a"]});
  await navigate(page, "运行诊断");

  await page.getByRole("form", { name: "路由解释" }).getByRole("combobox", { name: "route_id" }).selectOption("rt-multi-provider");
  await page.getByLabel("请求模型").fill("minimax-m3");
  await page.getByRole("button", { name: "解释" }).click();

  // Not "Management operation failed": the panel names the fix.
  const explainCard = page.locator(".rt-card", { hasText: "路由解释" });
  await expect(explainCard).toContainText("需要显式指定 Provider");
  await expect(explainCard).toContainText("必须显式选一个");

  await page.getByRole("form", { name: "路由解释" }).getByLabel("provider_id", { exact: false }).selectOption("prov-a");
  await page.getByRole("button", { name: "解释" }).click();
  await expect(page.locator('.rt-chip[data-state="dominant"]')).toBeVisible();
});

test("explain offers all three contract protocols", async ({ page }) => {
  await unlock(page);
  await selectActive(page);
  await resourceChoices(page,{routes:["rt-minimax","rt-multi-provider"],upstreams:["prov-a"]});
  await navigate(page, "运行诊断");

  // openai_chat_completions was absent until 2026-08-18: the drift gate cannot
  // see a literal a page omits, so Explain simply could not run that path.
  await expect(page.getByLabel("协议").locator("option")).toHaveText([
    "openai_chat_completions",
    "openai_responses",
    "anthropic_messages",
  ]);
});

test("legacy route policies expose safe details without the unsupported route reader", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async () => {
    const path = "/src/generated/management-client.ts";
    const { ManagementApi } = await import(path);
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation === "getRoute") throw new Error("Legacy inspector must not call getRoute");
      const response = await original.call(this, operation, request);
      if (operation !== "listRoutes" || !response.ok) return response;
      const body = await response.json();
      body.items.push(...["round_robin", "priority_failover"].map((policy) => ({
        id: `legacy-${policy}`, public_model_id: "pm-legacy", policy,
        max_attempts: 3, bootstrap_timeout_ms: 4500,
      })));
      return new Response(JSON.stringify(body), { status: 200, headers: response.headers });
    };
  });
  await navigate(page, "模型与路由");
  await page.getByText("高级路由、候选与别名", { exact: true }).click();
  const inventory = page.getByRole("region", { name: "完整配置资源" });
  for (const policy of ["round_robin", "priority_failover"]) {
    await inventory.locator("tr", { hasText: `legacy-${policy}` }).getByRole("button", { name: "打开路由" }).click();
    const inspector = page.getByRole("dialog", { name: "路由详情" });
    await expect(inspector).toContainText(policy);
    await expect(inspector).toContainText("4500");
    await expect(inspector).toContainText("pm-legacy");
    await page.keyboard.press("Escape");
    await expect(inspector).toHaveCount(0);
  }
});

test("inline candidate owns dirty departure and busy sibling actions",async({page})=>{
 await openModels(page);await makeRoute(page,"rt-inline-guard");
 await page.locator('.route-workbench').getByRole('button',{name:'打开它'}).click();await page.getByRole('button',{name:'加候选',exact:true}).click();
 const editor=page.getByRole('region',{name:'添加模型来源',exact:true});
 await editor.getByLabel('候选标识').fill('candidate-inline-guard');
 await page.getByRole('button',{name:'接入模型',exact:true}).click();
 await page.getByRole('dialog',{name:'放弃未保存的修改？'}).getByRole('button',{name:'继续编辑'}).click();
 await expect(editor.getByLabel('候选标识')).toHaveValue('candidate-inline-guard');
 await editor.getByRole('combobox',{name:'接口连接',exact:true}).selectOption('ep-relay-a-responses');await editor.getByLabel('上游原始模型 ID',{exact:true}).fill('exact-model');
 for(const size of [{width:1440,height:900},{width:1280,height:720},{width:390,height:844}]){await page.setViewportSize(size);expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)).toBe(false);}
 await page.evaluate(async()=>{const {ManagementApi}=await import('/src/generated/management-client.ts');const original=ManagementApi.prototype.request;ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){if(operation==='createRouteCandidate')await new Promise<void>(resolve=>Reflect.set(globalThis,'__releaseCandidate',resolve));return original.call(this,operation,request);};});
 await editor.getByRole('button',{name:'创建候选'}).click();await page.waitForFunction(()=>typeof Reflect.get(globalThis,'__releaseCandidate')==='function');
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'接入模型',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect(editor.getByRole('button',{name:'取消',exact:true})).toBeDisabled();
 await page.evaluate(()=>Reflect.get(globalThis,'__releaseCandidate')());await expect(page.getByRole('region',{name:'候选配置结果',exact:true})).toContainText('已保存到当前草稿');
});
