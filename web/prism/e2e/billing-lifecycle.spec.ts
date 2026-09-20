import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openBilling(page:Page){
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"计费与价格");
  await expect(page.getByRole("region",{name:"全局价格目录"})).toBeVisible();
}

for (const count of [1, 50, 51, 512]) {
  test(`all ${count} catalog entries remain reviewable and are submitted after pagination`, async ({ page }) => {
    await openBilling(page);
    await page.getByRole("button", { name: "导入目录" }).click();
    const form = page.getByRole("region", { name: "导入价格目录" });
    await form.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
    await form.getByText("目录信息与对比基线", { exact: true }).click();
    const id = await form.getByRole("textbox", { name: "目录版本 ID" }).inputValue();
    await form.getByRole("button", { name: "高级 JSON" }).click();
    const entries = Array.from({ length: count }, (_, index) => ({
      provider_id: "relay-a", channel_id: "ep-relay", model: `Exact/Price-${index + 1}`,
      input_microunits_per_million: index, output_microunits_per_million: index + 1,
      reasoning_microunits_per_million: 0, cache_read_microunits_per_million: 0,
      cache_creation_microunits_per_million: 0, cached_microunits_per_million: 0,
    }));
    await form.getByRole("textbox", { name: "完整价格条目" }).fill(JSON.stringify(entries));
    await form.getByRole("button", { name: "预览差异" }).click();
    const preview = page.getByRole("region", { name: "核对目录价格" });
    await expect(preview.locator(".price-preview-list > details")).toHaveCount(Math.min(50, count));
    const lastPage = Math.ceil(count / 50) - 1;
    await preview.getByRole("combobox", { name: "价格预览页码" }).selectOption(String(lastPage));
    await preview.locator("summary", { hasText: `Exact/Price-${count}` }).click();
    await expect(preview.locator("details[open] dt")).toHaveText(["输入", "输出", "推理", "缓存读", "缓存写", "已缓存"]);
    await expect(preview.getByRole("button", { name: "下一页" })).toBeDisabled();
    await preview.getByRole("button", { name: `价格差异（${count}）` }).click();
    await expect(preview.getByRole("combobox", { name: "价格预览页码" })).toHaveValue("0");
    await preview.getByRole("combobox", { name: "价格预览页码" }).selectOption(String(lastPage));
    await expect(preview.locator("summary", { hasText: `Exact/Price-${count}` })).toBeVisible();
    await page.locator(".inline-workspace").getByRole("button", { name: "确认导入" }).click();
    await expect(page.getByRole("region", { name: "目录导入结果" })).toContainText("全局价格目录已保存");
    const stored = await page.evaluate(async target => {
      const { call } = await import("/src/api/client.ts");
      return (await call<{ catalog_version_id: string; entries: unknown[] }[]>("listBillingCatalogs", {}, { versionScoped: true }))
        .find(row => row.catalog_version_id === target)?.entries;
    }, id);
    expect(stored).toEqual(entries);
  });
}

test("policy requires an explicit effective catalog and changes only the selected draft",async({page})=>{
  await openBilling(page);
  await page.getByText("高级路由价格策略",{exact:true}).click();
  const panel=page.locator(".bill-policy");
  await expect(panel).toContainText("正常的未配置状态");
  await panel.getByRole("button",{name:"设置策略"}).click();
  const dialog=page.getByRole("dialog",{name:"绑定路由价格目录"});
  const selector=dialog.getByRole("combobox",{name:"已生效的价格目录"});
  await expect(selector).toHaveValue("");
  await expect(selector.locator('option[value="cat-2026-09-preview"]')).toHaveCount(0);
  await expect(dialog.getByRole("button",{name:"保存到草稿"})).toBeDisabled();
  await selector.selectOption("cat-2026-08");
  await dialog.getByRole("button",{name:"保存到草稿"}).click();
  const result=page.getByRole("dialog",{name:"价格策略结果"});
  await expect(result).toContainText("已保存到草稿");
  await result.getByRole("button",{name:"完成"}).click();
  await expect(panel).toContainText("按费率比较");
  const bound=await page.evaluate(async()=>{const {call}=await import("/src/api/client.ts");return await call<{catalog_version_id:string}>("getRoutingPricePolicy",{},{versionScoped:true});});
  expect(bound.catalog_version_id).toBe("cat-2026-08");
  await panel.getByRole("button",{name:"清除策略"}).click();
  const clear=page.getByRole("dialog",{name:"清除路由价格策略"});
  await expect(clear).toContainText("目录和账本保留");
  await clear.getByRole("button",{name:"确认清除"}).click();
  await page.getByRole("dialog",{name:"价格策略结果"}).getByRole("button",{name:"完成"}).click();
  await expect(panel).toContainText("正常的未配置状态");
  await expect(page.getByRole("region",{name:"全局价格目录"}).locator("tbody tr")).toHaveCount(3);
});

test("copy import saves a global catalog without claiming the draft was published",async({page})=>{
  await openBilling(page);
  const catalogRow=page.locator('[data-resource-id="cat-2026-08"]');
  await catalogRow.getByRole("button",{name:"复制编辑"}).click();
  const form=page.getByRole("region",{name:"复制现有目录并调整"});
  await form.getByText("目录信息与对比基线",{exact:true}).click();
  const id=await form.getByRole("textbox",{name:"目录版本 ID"}).inputValue();
  await form.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await form.getByRole("button",{name:"预览差异"}).click();
  const preview=page.getByRole("region",{name:"确认完整价格目录"});
  await expect(preview.locator("#billing-catalog-import-form")).toBeHidden();
  await expect(preview).toContainText(id);
  await expect(preview).toContainText("UTC");
  await preview.getByRole("button",{name:"确认导入"}).click();
  const result=page.getByRole("region",{name:"目录导入结果"});
  await expect(result).toContainText("全局价格目录已保存");
  await expect(result).toContainText("工作配置尚未发布");
  await result.getByRole("button",{name:"完成"}).click();
  const global=await page.evaluate(async(target)=>{
    const {call}=await import("/src/api/client.ts");
    const catalogs=await call<{catalog_version_id:string}[]>("listBillingCatalogs",{headers:{"X-Config-Version":"v-2026-07"}});
    return catalogs.some(row=>row.catalog_version_id===target);
  },id);
  expect(global).toBe(true);
  await expect(page.locator(".dock")).toContainText("草稿");
});

test("restore appends an operator catalog and retains the predecessor",async({page})=>{
  await openBilling(page);
  await page.locator('[data-resource-id="cat-2026-07"]').getByRole("button",{name:"恢复价格"}).click();
  const form=page.getByRole("dialog",{name:"从历史目录恢复价格"});
  await form.getByText("新目录技术标识",{exact:true}).click();
  const id=await form.getByRole("textbox",{name:"新目录版本 ID"}).inputValue();
  await form.getByLabel("生效时间（本地时区）").fill("2026-09-19T09:00");
  await form.getByRole("button",{name:"核对恢复内容"}).click();
  const preview=page.getByRole("dialog",{name:"确认创建恢复目录"});
  await expect(preview).toContainText("不会回滚配置、删除中间目录或改写账本");
  await preview.getByRole("button",{name:"创建新目录"}).click();
  await page.getByRole("dialog",{name:"价格恢复结果"}).getByRole("button",{name:"完成"}).click();
  const rows=await page.evaluate(async(target)=>{
    const {call}=await import("/src/api/client.ts");
    const all=await call<{catalog_version_id:string;source:string;entries:unknown[]}[]>("listBillingCatalogs",{},{versionScoped:true});
    return {before:all.find(row=>row.catalog_version_id==="cat-2026-07"),after:all.find(row=>row.catalog_version_id===target)};
  },id);
  expect(rows.after?.source).toBe("operator");
  expect(rows.after?.entries).toEqual(rows.before?.entries);
});

test("invalid or oversized JSON stays intact and never dispatches a catalog write",async({page})=>{
  await openBilling(page);
  await page.getByRole("button",{name:"导入目录"}).click();
  const dialog=page.getByRole("region",{name:"导入价格目录"});
  await dialog.getByRole("button",{name:"高级 JSON"}).click();
  const raw=dialog.getByRole("textbox",{name:"完整价格条目"});
  await raw.fill("[{");
  await dialog.getByRole("button",{name:"表格编辑"}).click();
  await expect(dialog.getByRole("alert")).toContainText("原文已保留");
  await expect(raw).toHaveValue("[{");
  const oversized=JSON.stringify(Array.from({length:513},(_,index)=>({model:`Model-${index}`})));
  await raw.fill(oversized);
  await dialog.getByRole("button",{name:"表格编辑"}).click();
  await expect(dialog.getByRole("alert")).toContainText("1–512");
  await expect(raw).toHaveValue(oversized);
});

test("lost import response retains exact catalog and draft without replay",async({page})=>{
  await openBilling(page);
  await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"复制编辑"}).click();
  const form=page.getByRole("region",{name:"复制现有目录并调整"});
  await form.getByText("目录信息与对比基线",{exact:true}).click();
  const id=await form.getByRole("textbox",{name:"目录版本 ID"}).inputValue();
  await form.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await form.getByRole("button",{name:"预览差异"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__billingImportWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="importBillingCatalog")return original.call(this,operation,request);
      Reflect.set(globalThis,"__billingImportWrites",Number(Reflect.get(globalThis,"__billingImportWrites"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic catalog response lost");
    };
  });
  await page.getByRole("region",{name:"确认完整价格目录"}).getByRole("button",{name:"确认导入"}).click();
  const result=page.getByRole("region",{name:"目录导入结果"});
  await expect(result).toContainText("未确认");
  await expect(result).toContainText(id);
  await expect(result).toContainText("draft-2026-08");
  await result.getByRole("button",{name:"核对工作配置"}).click();
  await expect(page).toHaveURL(/#\/versions/u);
  await expect(page.locator("main.canvas")).toHaveAttribute("data-context-version","draft-2026-08");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__billingImportWrites")))).toBe(1);
});

test("active-context catalog append remains globally saved without publication",async({page})=>{
  await unlock(page);
  await navigate(page,"计费与价格");
  await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"复制编辑"}).click();
  const form=page.getByRole("region",{name:"复制现有目录并调整"});
  await form.getByText("目录信息与对比基线",{exact:true}).click();
  const id=await form.getByRole("textbox",{name:"目录版本 ID"}).inputValue();
  await form.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await form.getByRole("button",{name:"预览差异"}).click();
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__billingPublicationCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="publishConfigVersion"){
        Reflect.set(globalThis,"__billingPublicationCalls",Number(Reflect.get(globalThis,"__billingPublicationCalls"))+1);
        throw new Error("synthetic publish unavailable");
      }
      return original.call(this,operation,request);
    };
  });
  await page.getByRole("region",{name:"确认完整价格目录"}).getByRole("button",{name:"确认导入"}).click();
  const receipt=page.getByRole("region",{name:"目录导入结果"});
  await expect(receipt).toContainText("工作配置尚未发布");
  await expect(receipt).toContainText(id);
  await receipt.getByRole("button",{name:"完成",exact:true}).click();
  await expect(page.locator("main")).toHaveAttribute("data-context-status","draft");
  const persisted=await page.evaluate(async(target)=>{
    const {call}=await import("/src/api/client.ts");
    const rows=await call<{catalog_version_id:string}[]>("listBillingCatalogs",{},{versionScoped:true});
    return rows.filter(row=>row.catalog_version_id===target).length;
  },id);
  expect(persisted).toBe(1);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__billingPublicationCalls")))).toBe(0);
});

test("lost policy response shows one non-replayable draft receipt",async({page})=>{
  await openBilling(page);
  await page.getByText("高级路由价格策略",{exact:true}).click();
  await page.locator(".bill-policy").getByRole("button",{name:"设置策略"}).click();
  const editor=page.getByRole("dialog",{name:"绑定路由价格目录"});
  await editor.getByRole("combobox",{name:"已生效的价格目录"}).selectOption("cat-2026-08");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__billingPolicyWrites",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="setRoutingPricePolicy")return original.call(this,operation,request);
      Reflect.set(globalThis,"__billingPolicyWrites",Number(Reflect.get(globalThis,"__billingPolicyWrites"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic policy response lost");
    };
  });
  await editor.getByRole("button",{name:"保存到草稿"}).click();
  const receipt=page.getByRole("dialog",{name:"价格策略结果"});
  await expect(receipt).toContainText("结果未确认");
  await expect(receipt).toContainText("cat-2026-08");
  await receipt.getByRole("button",{name:"核对草稿"}).click();
  await expect(page).toHaveURL(/#\/versions/u);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__billingPolicyWrites")))).toBe(1);
});

test("advisory source quotes retain missing rates and do not import a catalog",async({page})=>{
  await openBilling(page);
  await page.evaluate(async()=>{const {trackFixtureOperationForTest}=await import("/src/dev/fixtures.ts");trackFixtureOperationForTest("POST /admin/billing/catalogs");});
  await page.getByRole("button",{name:"导入目录"}).click();
  const dialog=page.getByRole("region",{name:"导入价格目录"});
  await dialog.getByRole("combobox",{name:"接口连接"}).selectOption("ep-relay-a-responses");
  await dialog.getByRole("textbox",{name:"计费模型名（已解析的公开模型）"}).fill("Exact/Model");
  await dialog.getByRole("button",{name:"从 models.dev 读取价格"}).click();
  await expect(dialog).toContainText("匹配 2 份来源报价");
  const quote=dialog.getByRole("combobox",{name:"选择来源报价"});
  await expect(quote).toBeDisabled();
  await dialog.getByRole("checkbox",{name:/本次目录采用 USD/u}).check();
  await expect(quote.locator("option",{hasText:"分段价格"})).toHaveAttribute("disabled","");
  await quote.selectOption("0");
  await expect(dialog.getByRole("spinbutton",{name:"输入"})).toHaveValue("0");
  await expect(dialog.getByRole("spinbutton",{name:"输出"})).toHaveValue("1250000");
  await expect(dialog.getByRole("spinbutton",{name:"推理"})).toHaveValue("");
  expect(await page.evaluate(async()=>{const {fixtureOperationCallsForTest}=await import("/src/dev/fixtures.ts");return fixtureOperationCallsForTest("POST /admin/billing/catalogs");})).toBe(0);
});

test("table pagination preserves edits and locates an invalid off-page rate", async ({page}) => {
  await openBilling(page);
  await page.getByRole("button",{name:"导入目录",exact:true}).click();
  const dialog=page.locator(".inline-workspace");
  await dialog.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await dialog.getByRole("button",{name:"高级 JSON"}).click();
  const rows=Array.from({length:51},(_,i)=>({provider_id:"relay-a",channel_id:"ep-relay",model:`Paged-${i}`,input_microunits_per_million:0,output_microunits_per_million:i===50?null:1,reasoning_microunits_per_million:0,cache_read_microunits_per_million:0,cache_creation_microunits_per_million:0,cached_microunits_per_million:0}));
  await dialog.getByLabel("完整价格条目").fill(JSON.stringify(rows));
  await dialog.getByRole("button",{name:"表格编辑"}).click();
  await expect(dialog.locator("fieldset.price-entry")).toHaveCount(50);
  await dialog.getByRole("button",{name:"预览差异"}).click();
  await expect(dialog.getByRole("alert")).toContainText("第 51 条");
  await expect(dialog.getByRole("combobox",{name:"编辑页码"})).toHaveValue("1");
  await expect(dialog.getByRole("spinbutton",{name:"输出",exact:true})).toBeFocused();
  await dialog.getByRole("spinbutton",{name:"输出",exact:true}).fill("7");
  await dialog.getByRole("combobox",{name:"编辑页码"}).selectOption("0");
  await dialog.getByRole("button",{name:"高级 JSON"}).click();
  const result=JSON.parse(await dialog.getByLabel("完整价格条目").inputValue());
  expect(result).toHaveLength(51);
  expect(result[50].output_microunits_per_million).toBe(7);
  expect(result[0]).toEqual(rows[0]);
  await dialog.getByRole("button",{name:"预览差异"}).click();
  await expect(page.getByRole("heading",{name:"确认完整价格目录"})).toBeVisible();
});

test("disjoint full catalogs expose all 1024 differences",async({page})=>{
  await openBilling(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const entries=Array.from({length:512},(_,index)=>({provider_id:"relay-a",channel_id:"ep-relay",model:`Before-${index}`,input_microunits_per_million:0,output_microunits_per_million:1,reasoning_microunits_per_million:0,cache_read_microunits_per_million:0,cache_creation_microunits_per_million:0,cached_microunits_per_million:0}));
    await call("importBillingCatalog",{body:{catalog_version_id:"catalog-diff-full",effective_at_ms:1000,source:"operator",entries}},{versionScoped:true,mutating:true});
  });
  await navigate(page,"仪表盘");await navigate(page,"计费与价格");
  await page.locator('[data-resource-id="catalog-diff-full"]').getByRole("button",{name:"复制编辑"}).click();
  const editor=page.locator(".inline-workspace");
  await editor.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
  await editor.getByRole("button",{name:"高级 JSON"}).click();
  const field=editor.getByLabel("完整价格条目");
  const entries=JSON.parse(await field.inputValue()).map((row:{model:string})=>({...row,model:row.model.replace("Before-","After-")}));
  await field.fill(JSON.stringify(entries));
  await editor.getByRole("button",{name:"预览差异"}).click();
  const preview=page.getByRole("region",{name:"核对目录价格"});
  await preview.getByRole("button",{name:"价格差异（1024）"}).click();
  let count=0;
  for(let index=0;index<21;index++){
    await preview.getByRole("combobox",{name:"价格预览页码"}).selectOption(String(index));
    count+=await preview.locator(".price-preview-list > details").count();
  }
  expect(count).toBe(1024);
  await expect(preview.getByRole("button",{name:"下一页"})).toBeDisabled();
});

test("catalog capacity blocks both append entry points and preserves readable history",async({page})=>{
  await openBilling(page);
  await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    const rows=await call<Array<{entries:unknown[]}>>("listBillingCatalogs",{},{versionScoped:true});
    for(let index=rows.length;index<256;index++)await call("importBillingCatalog",{body:{catalog_version_id:`capacity-${index}`,effective_at_ms:1000,source:"operator",entries:rows[0].entries}},{versionScoped:true,mutating:true});
  });
  await navigate(page,"仪表盘");await navigate(page,"计费与价格");
  await expect(page.getByRole("button",{name:"导入目录",exact:true})).toBeDisabled();
  await expect(page.getByRole("status").filter({hasText:"256 份上限"})).toBeVisible();
  await expect(page.locator('.bill-catalog-table tbody tr').first().getByRole("button",{name:"恢复价格"})).toBeDisabled();
  const observed=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const before=useVersionStore.getState().context?.revision;
    const rows=await call<Array<{entries:unknown[]}>>("listBillingCatalogs",{},{versionScoped:true});
    let rejected=false;try{await call("importBillingCatalog",{body:{catalog_version_id:"over-capacity",effective_at_ms:1000,source:"operator",entries:rows[0].entries}},{versionScoped:true,mutating:true});}catch{rejected=true;}
    return {rejected,before,after:useVersionStore.getState().context?.revision,count:(await call<unknown[]>("listBillingCatalogs",{},{versionScoped:true})).length};
  });
  expect(observed.rejected).toBe(true);expect(observed.count).toBe(256);expect(observed.after).toBe(observed.before);
});
