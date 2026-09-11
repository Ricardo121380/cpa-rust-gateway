import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("an unbound account is discoverable and can be disabled without resubmitting its secret", async ({page}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", {name: "添加账号", exact: true}).click();
  const dialog = page.getByRole("dialog", {name: "添加账号"});
  await dialog.getByLabel("提供商").selectOption("relay-a");
  await dialog.getByLabel("账号名称").fill("team-unbound");
  await dialog.getByLabel("API Key / Token", {exact: true}).fill("synthetic-private-token");
  await dialog.getByRole("button", {name: "添加账号", exact: true}).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("textbox", {name: "搜索账号", exact: true}).fill("team-unbound");
  const row = page.locator("tbody tr");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText("未绑定");
  await expect(row).not.toContainText("synthetic-private-token");
  await row.getByRole("button", {name: "停用", exact: true}).click();
  const confirm = page.getByRole("dialog", {name: "停用账号"});
  await expect(confirm.locator("input")).toHaveCount(0);
  await confirm.getByRole("button", {name: "确认更改"}).click();
  await expect(confirm).toHaveCount(0);
  await expect(row).toContainText("已停用");
  await row.getByRole("link", {name: "未绑定", exact: true}).click();
  await expect(page.locator(".subresource-panel")).toContainText("team-unbound");
});

test("Codex reauthorization is directly accessible while bearer accounts cannot enter it", async ({page}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("textbox", {name: "搜索账号", exact: true}).fill("cred-relay-key");
  await expect(page.locator("tbody tr")).toHaveCount(1);
  await expect(page.getByRole("button", {name: "重新授权", exact: true})).toHaveCount(0);
  await page.getByRole("textbox", {name: "搜索账号", exact: true}).fill("cred-codex-oauth");
  await page.getByRole("button", {name: "重新授权", exact: true}).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

for (const width of [1440, 390]) {
  test(`managed account layout at ${width}px`, async ({page}) => {
    await page.setViewportSize({width, height: width === 390 ? 844 : 900});
    await unlock(page);
    await selectDraft(page);
    await navigate(page, "账号池");
    await expect(page.locator(width === 390 ? ".managed-account-cards article" : "tbody tr").first()).toBeVisible();
    await expect(page.getByRole("button", {name: "添加账号", exact: true})).toBeVisible();
    await page.screenshot({path: `../../output/managed-accounts-${width}.png`, fullPage: true});
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });
}

test("channel chooser covers the agreed families and imports Codex from a file", async ({page}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", {name: "添加账号", exact: true}).click();
  const dialog = page.getByRole("dialog", {name: "添加账号"});
  const channel = dialog.getByRole("combobox", {name: "渠道", exact: true});
  await expect(channel.locator("option")).toHaveCount(9);
  for (const id of ["grok.build", "grok.console", "grok.web"]) {
    await channel.selectOption(id);
    await expect(dialog.locator("textarea")).toHaveCount(1);
    await expect(dialog.getByLabel("提供商", {exact:true})).toHaveCount(0);
  }
  await channel.selectOption("codex");
  await dialog.getByLabel("账号名称").fill("codex-file-import");
  const document = JSON.stringify({kind:"codex_oauth", access_token:"synthetic-file-token",refresh_token:"synthetic-file-refresh",expires_at_ms:4102444800000,account_id:"synthetic-account"});
  await dialog.getByLabel("读取凭据文件").setInputFiles({name:"synthetic.json",mimeType:"application/json",buffer:Buffer.from(document)});
  await expect(dialog.locator("textarea")).toHaveValue(document);
  await dialog.getByRole("button", {name:"添加账号",exact:true}).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole("textbox",{name:"搜索账号",exact:true}).fill("codex-file-import");
  await expect(page.locator("tbody tr")).toContainText("Codex OAuth");
  await expect(page.locator("main")).not.toContainText("synthetic-file-token");
});
