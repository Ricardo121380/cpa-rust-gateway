import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function openAccounts(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await expect(page.getByRole("button", { name: "授权 / 导入账号", exact: true })).toBeVisible();
}

test("account onboarding opens the channel-owned chooser without provider selection", async ({ page }) => {
  await openAccounts(page);
  await page.getByRole("button", { name: "授权 / 导入账号", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "授权或导入账号" });
  await expect(dialog.getByLabel("渠道", { exact: true })).toBeVisible();
  await expect(dialog.getByLabel("提供商", { exact: true })).toHaveCount(0);
  await expect(dialog.getByLabel("接口连接", { exact: true })).toHaveCount(0);
});

test("Codex reauthorization is available only for the observed Codex authorization", async ({ page }) => {
  await openAccounts(page);
  await page.getByRole("textbox", { name: "搜索账号", exact: true }).fill("alex@example.test");
  const row = page.locator(".account-list tbody tr");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText("OAuth 授权");
  await row.getByRole("button", { name: "重新授权", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

for (const width of [1440, 390]) {
  test(`managed account entry and rows fit at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    await openAccounts(page);
    await expect(page.locator(".account-list tbody tr").first()).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });
}

test("channel chooser exposes the supported channel families", async ({ page }) => {
  await openAccounts(page);
  await page.getByRole("button", { name: "授权 / 导入账号", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "授权或导入账号" });
  const channel = dialog.getByRole("combobox", { name: "渠道", exact: true });
  await expect(channel.locator('option[value="codex"]')).toHaveCount(1);
  for (const id of ["grok.build", "grok.console", "grok.web"]) {
    await channel.selectOption(id);
    await expect(dialog.getByLabel("提供商", { exact: true })).toHaveCount(0);
  }
});

test("batch maintenance reports a saved draft per exact selected authorization", async ({ page }) => {
  await openAccounts(page);
  await page.getByRole("button", { name: "批量管理", exact: true }).click();
  await page.locator(".account-list").getByRole("checkbox").first().check();
  await page.getByRole("button", { name: "停用", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "停用 1 份授权" });
  await dialog.getByRole("button", { name: "确认停用", exact: true }).click();
  await expect(dialog).toContainText("已保存到草稿");
  await dialog.getByRole("button", { name: "完成", exact: true }).click();
});

test("an unbound API import is discoverable and can be disabled without resubmitting its material", async ({ page }) => {
  await openAccounts(page);
  await page.evaluate(() => { Object.defineProperty(crypto, "randomUUID", {configurable:true,value:()=>"team-unbound"}); });
  await page.getByRole("button", { name: "授权 / 导入账号", exact: true }).click();
  const dialog=page.getByRole("dialog", { name: "授权或导入账号" });
  await dialog.getByLabel("渠道", {exact:true}).selectOption("openai-compatible");
  const service=dialog.getByRole("combobox", {name:"服务",exact:true});
  await expect(service).toBeVisible();
  await service.selectOption({label:"中转站 A"});
  await dialog.getByLabel("API Key / Token", {exact:true}).fill("synthetic-private-token");
  await dialog.getByRole("button", {name:"导入账号",exact:true}).click();
  await expect(dialog).toContainText("导入结果");
  await expect(dialog).toContainText("已添加");
  await expect(dialog).not.toContainText("synthetic-private-token");
  await dialog.getByRole("button", {name:"完成",exact:true}).click();
  const inventory=await page.evaluate(async()=>{
    const {call}=await import("/src/api/client.ts");
    return call<{items:readonly {id:string;managed:unknown;native_account:unknown}[]}>("listAccountInventory",{query:{limit:50}},{versionScoped:true});
  });
  expect(inventory.items.some((item)=>item.id==="import-team-unbound"&&item.managed!==null&&item.native_account===null)).toBe(true);
  const unbound=page.locator('[data-account-key="import-team-unbound"]');
  await expect(unbound).toBeVisible({timeout:15_000});
  await expect(unbound).toContainText("未连接接口");
  await unbound.getByRole("button", {name:"更多",exact:true}).click();
  await page.getByRole("dialog", {name:"账号操作"}).getByRole("button", {name:"停用账号",exact:true}).click();
  const confirm=page.getByRole("dialog", {name:"停用 1 份授权"});
  await expect(confirm.locator("input")).toHaveCount(0);
  await confirm.getByRole("button", {name:"确认停用",exact:true}).click();
  await expect(confirm).toContainText("已保存到草稿");
  await confirm.getByRole("button", {name:"完成",exact:true}).click();
  await expect(unbound).toContainText("已停用");
  await expect(page.locator("body")).not.toContainText("synthetic-private-token");
});

test("channel-owned Codex file import stays secret-free and API accounts cannot reauthorize", async ({ page }) => {
  await openAccounts(page);
  const api=page.locator('.account-group[aria-label="API 账号"]');
  await expect(api.getByRole("button", {name:"重新授权",exact:true})).toHaveCount(0);
  await page.getByRole("button", { name: "授权 / 导入账号", exact: true }).click();
  const dialog=page.getByRole("dialog", { name: "授权或导入账号" });
  await dialog.getByLabel("渠道", {exact:true}).selectOption("codex");
  await dialog.getByRole("button", {name:"选择文件",exact:true}).click();
  const material=JSON.stringify({kind:"codex_oauth",access_token:"fixture-token",refresh_token:"fixture-refresh",expires_at_ms:4102444800000,account_id:"synthetic-account"});
  await dialog.getByLabel("凭据文件", {exact:true}).setInputFiles({name:"synthetic.json",mimeType:"application/json",buffer:Buffer.from(material)});
  await expect(dialog).toContainText("synthetic.json");
  await dialog.getByRole("button", {name:"导入账号",exact:true}).click();
  await expect(dialog).toContainText("导入结果");
  await expect(dialog).toContainText("已添加");
  await expect(dialog).not.toContainText("fixture-token");
  await dialog.getByRole("button", {name:"完成",exact:true}).click();
  await expect(page.locator("main")).not.toContainText("fixture-token");
});

test("a rejected channel import reports a failed item rather than a successful result", async ({ page }) => {
  await openAccounts(page);
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation !== "importChannelAccount") return original.call(this, operation, request);
      return new Response(JSON.stringify({error:{code:"fixture_import_rejected",message:"synthetic rejection"}}), {status:400,headers:{"Content-Type":"application/json"}});
    };
  });
  await page.getByRole("button", {name:"授权 / 导入账号",exact:true}).click();
  const dialog=page.getByRole("dialog", {name:"授权或导入账号"});
  await dialog.getByLabel("渠道", {exact:true}).selectOption("openai-compatible");
  await dialog.getByRole("combobox", {name:"服务",exact:true}).selectOption({label:"中转站 A"});
  await dialog.getByLabel("API Key / Token", {exact:true}).fill("synthetic");
  await dialog.getByRole("button", {name:"导入账号",exact:true}).click();
  await expect(dialog).toContainText("导入结果");
  await expect(dialog).toContainText("未添加");
  await expect(dialog).not.toContainText("已添加");
});
