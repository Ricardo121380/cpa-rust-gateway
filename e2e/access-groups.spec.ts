import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// Access groups and route grants — the half of the configuration chain Prism
// could not do. Without a group and a grant, an issued Client Key reaches no
// model at all, so these are not conveniences.

async function openAccess(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
}

test("a group can be created, edited and deleted", async ({ page }) => {
  await openAccess(page);

  await page.getByRole("button", { name: "新建访问组" }).click();
  const create = page.getByRole("dialog");
  await create.getByLabel("ID").fill("team-e2e");
  await create.getByLabel("名称").fill("端到端组");
  await create.getByLabel("限制").fill("max_concurrency=8 rpm=120");
  await create.getByRole("button", { name: "创建" }).click();

  const row = page.locator("tr", { hasText: "team-e2e" }).first();
  await expect(row).toContainText("端到端组");
  await expect(row).toContainText("max_concurrency=8 rpm=120");

  // Edit is a full replacement — the form must arrive pre-filled or the save
  // would silently blank the fields the operator did not touch.
  await row.getByRole("button", { name: "编辑" }).click();
  const edit = page.getByRole("dialog");
  await expect(edit.getByLabel("ID")).toHaveValue("team-e2e");
  await expect(edit.getByLabel("名称")).toHaveValue("端到端组");
  await expect(edit.getByLabel("限制")).toHaveValue("max_concurrency=8 rpm=120");
  await edit.getByLabel("名称").fill("改名后");
  await edit.getByRole("button", { name: "保存" }).click();
  await expect(page.locator("tr", { hasText: "team-e2e" }).first()).toContainText("改名后");

  await page.locator("tr", { hasText: "team-e2e" }).first().getByRole("button", { name: "删除" }).click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("会同时移除它的路由授权");
  await confirm.getByRole("button", { name: "确认删除" }).click();
  await expect(page.locator("tr", { hasText: "team-e2e" })).toHaveCount(0);
});

test("limits are judged before they reach the gateway", async ({ page }) => {
  await openAccess(page);
  await page.getByRole("button", { name: "新建访问组" }).click();
  const create = page.getByRole("dialog");
  await create.getByLabel("ID").fill("team-bad");
  await create.getByLabel("名称").fill("坏限额");
  await create.getByLabel("限制").fill("rpm=-1");
  await create.getByRole("button", { name: "创建" }).click();

  await expect(page.getByRole("alert")).toContainText("非负整数");
  // the sheet stays open and nothing was created
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("route grants are listed per group and can be added", async ({ page }) => {
  await openAccess(page);

  const row = page.locator("tr", { hasText: "team-default" }).first();
  await row.getByRole("button", { name: "路由" }).click();
  const routes = page.locator(".group-routes");
  await expect(routes).toContainText("rt-minimax");

  await routes.getByRole("button", { name: "授权路由" }).click();
  const sheet = page.getByRole("dialog");
  // Routes are not enumerable — the field says so rather than pretending the
  // suggestion list is complete.
  await expect(sheet).toContainText("契约没有 listRoutes");
  await sheet.getByLabel("route_id").fill("rt-e2e");
  await sheet.getByRole("button", { name: "授权" }).click();
  await expect(page.locator(".group-routes")).toContainText("rt-e2e");
});

test("a group with no grant says the keys under it reach nothing", async ({ page }) => {
  await openAccess(page);
  // team-batch exists with no grants seeded
  await page.locator("tr", { hasText: "team-batch" }).first().getByRole("button", { name: "路由" }).click();
  await expect(page.locator(".group-routes")).toContainText("到不了任何模型");
});

test("group editing is refused on a published version", async ({ page }) => {
  await unlock(page);
  await navigate(page, "访问控制");
  // v-2026-07 is active, not a draft
  await page.locator(".version-picker select").selectOption("v-2026-07");
  await expect(page.getByRole("button", { name: "新建访问组" })).toBeDisabled();
});
