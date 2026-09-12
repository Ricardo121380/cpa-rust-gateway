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
