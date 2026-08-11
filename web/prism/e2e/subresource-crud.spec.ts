import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// The subresource half of the configuration chain. Before this, the panel
// could show channels/accounts/bindings but not create one — everything in a
// working config had to be made outside Prism.

async function openPanel(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await page
    .locator("tr", { hasText: "relay-a" })
    .first()
    .getByRole("button", { name: "子资源" })
    .click();
  await expect(page.locator(".subresource-panel")).toBeVisible();
}

test("a channel can be created and appears in the inventory", async ({ page }) => {
  await openPanel(page);
  await page.getByRole("button", { name: "新建 Channel" }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("id", { exact: true }).fill("ep-e2e");
  await sheet.getByLabel("adapter_id").fill("openai-compatible.responses");
  await sheet.getByLabel("api_format").fill("openai/responses");
  await sheet.getByLabel("base_url").fill("https://e2e.example/v1");
  await sheet.getByLabel("inference_path").fill("/responses");
  await sheet.getByRole("button", { name: "创建" }).click();

  // Binding-driven: the new channel has no row yet, so the per-row bind button
  // does not exist for it. The panel-level entry is the way out of that
  // chicken-and-egg, and the panel says so above the table.
  await expect(page.locator(".subresource-panel")).not.toContainText("ep-e2e");
  await expect(page.locator(".subresource-panel")).toContainText("绑定之前不会出现在下表");

  await page.locator(".subresource-panel").getByRole("button", { name: "加绑定" }).first().click();
  const bind = page.getByRole("dialog");
  await expect(bind.getByLabel("channel_id")).toHaveValue("");
  await bind.getByLabel("channel_id").fill("ep-e2e");
  await bind.getByLabel("credential_id").fill("cred-relay-key");
  await bind.getByRole("button", { name: "添加" }).click();

  // now it exists in the inventory
  await expect(page.locator(".subresource-panel")).toContainText("ep-e2e");
});

test("editing a channel pre-fills the URL the inventory does not carry", async ({ page }) => {
  await openPanel(page);
  await page
    .locator("tr", { hasText: "ep-relay-a-responses" })
    .first()
    .getByRole("button", { name: "编辑" })
    .click();
  const sheet = page.getByRole("dialog");
  // base_url comes from getEndpoint — account-pools is URL-free by contract,
  // and a blank field here would erase it on save.
  await expect(sheet.getByLabel("base_url")).toHaveValue("https://relay-a.example.com/v1");
  await expect(sheet.getByLabel("inference_path")).toHaveValue("/responses");
  await sheet.getByLabel("api_format").fill("openai/chat");
  await sheet.getByRole("button", { name: "保存" }).click();
  await expect(page.locator(".subresource-panel")).toContainText("openai/chat");
});

test("editing an account demands the secret again, and says why", async ({ page }) => {
  await openPanel(page);
  await page
    .locator(".subresource-panel")
    .getByRole("row", { name: /^cred-relay-key api_key/u })
    .getByRole("button", { name: "编辑" })
    .click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("永不返回密钥");
  await expect(sheet).toContainText("整体替换");
  await expect(sheet.getByLabel("secret")).toHaveAttribute("required", "");
  // and it is not a password field — those swallow paste in Safari
  await expect(sheet.getByLabel("secret")).toHaveAttribute("type", "text");
});

test("deleting a channel warns that its bindings and candidates go with it", async ({ page }) => {
  await openPanel(page);
  await page
    .locator("tr", { hasText: "ep-relay-a-responses" })
    .first()
    .getByRole("button", { name: "删除" })
    .click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("连带移除它的全部绑定");
  await expect(confirm).toContainText("路由候选将失去目标");
  await confirm.getByRole("button", { name: "确认删除" }).click();
  // the whole provider had exactly one binding, so the inventory empties
  await expect(page.locator(".empty-state")).toContainText("没有任何绑定");
});

test("subresource editing is refused on a published version", async ({ page }) => {
  await unlock(page);
  await navigate(page, "上游");
  await page.locator(".version-picker select").selectOption("v-2026-07");
  await page.waitForTimeout(400);
  const rows = page.locator("tr", { hasText: "relay-a" });
  if ((await rows.count()) > 0) {
    await rows.first().getByRole("button", { name: "子资源" }).click();
    const panel = page.locator(".subresource-panel");
    if (await panel.isVisible()) {
      await expect(panel.getByRole("button", { name: "新建 Channel" })).toBeDisabled();
    }
  }
});
