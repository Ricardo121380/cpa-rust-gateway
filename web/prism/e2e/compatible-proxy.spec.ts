// 兼容出口 · 代理池 / 节点 / 绑定 (P13-11 A–D) on the egress page.
//
// The assertions target what makes these three resources different from the
// other CRUD on this app: a write-only endpoint whose update lifecycle is the
// inverse of the credential secret, a target id drawn from two different
// namespaces, and a pool that must stay reachable while it has no nodes.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

const PANEL = ".compatible-proxy";

async function open(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "出口策略");
  await expect(page.locator(PANEL)).toBeVisible();
}

test("a pool with no nodes stays visible and reachable", async ({ page }) => {
  await open(page);

  // pool-empty is the state every pool is in the moment it is created. If the
  // node view only rendered pools that have nodes, the pool you just made would
  // vanish — which is why the create button is on the section, not on a row.
  await expect(page.locator(PANEL).getByText("pool-empty").first()).toBeVisible();
  const section = page.locator(".cp-section", { hasText: "代理节点" });
  await expect(section.locator(".cp-group", { hasText: "pool-empty" })).toContainText(
    "这个池还没有节点",
  );
  await expect(section.getByRole("button", { name: "新建" })).toBeEnabled();
});

test("the node form refuses a bad proxy address before sending anything", async ({ page }) => {
  await open(page);

  const nodeSection = page.locator(".cp-section", { hasText: "代理节点" });
  await nodeSection.getByRole("button", { name: "新建" }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("id", { exact: true }).fill("node-new");
  await sheet.getByLabel("upstream").selectOption("relay-a");
  await sheet.getByLabel("名称").fill("测试节点");
  await sheet.getByLabel("proxy_endpoint").fill("http://user:pw@example.com/path");
  await sheet.getByRole("button", { name: "创建" }).click();

  // The first rule broken is named, and the sheet stays open holding the input.
  await expect(sheet.locator(".field-error")).toContainText("socks5");
  await expect(sheet).toBeVisible();

  // …and nothing was created. (Request interception cannot prove this: fixtures
  // are injected through options.fetch, so no request reaches the network layer
  // for Playwright to observe. The absence of the row is the observable fact.)
  await sheet.getByRole("button", { name: "取消" }).click();
  await expect(nodeSection.locator("tr", { hasText: "node-new" })).toHaveCount(0);
});

test("editing a node keeps the sealed address when the field is left blank", async ({ page }) => {
  await open(page);

  // Scoped to the node section: "node-eu-1" is also the target of a binding,
  // so an unscoped row match finds two rows.
  const nodeSection = page.locator(".cp-section", { hasText: "代理节点" });
  await nodeSection.locator("tr", { hasText: "node-eu-1" }).getByRole("button", { name: "编辑" }).click();
  const sheet = page.getByRole("dialog");
  // The OPPOSITE of the account sheet, which demands the secret again. The
  // contract says omitted or null preserves the sealed endpoint.
  await expect(sheet).toContainText("留空表示保留现有地址");
  await expect(sheet.getByLabel("proxy_endpoint")).toHaveValue("");
  await sheet.getByLabel("名称").fill("法兰克福 1 改名");
  await sheet.getByRole("button", { name: "保存" }).click();

  // The save SUCCEEDING is the proof that proxy_endpoint was omitted rather
  // than sent blank: the fixture validates any present endpoint the way the
  // gateway does, and "" fails that check, so a blank one would have 400ed.
  await expect(nodeSection.locator("tr", { hasText: "node-eu-1" })).toContainText("法兰克福 1 改名");
  await expect(page.locator(".action-error")).toHaveCount(0);
  await expect(nodeSection.locator("tr", { hasText: "node-eu-1" })).toContainText("已配置(封存)");
});

test("the target id switches namespace with the target kind", async ({ page }) => {
  await open(page);

  await page
    .locator(".cp-section", { hasText: "兼容出口绑定" })
    .getByRole("button", { name: "新建" })
    .click();
  const sheet = page.getByRole("dialog");

  // direct carries no id at all — the backend rejects direct + any id.
  await expect(sheet).toContainText("直连不带目标 id");
  await expect(sheet.getByLabel("target_id", { exact: false })).toHaveCount(0);

  await sheet.getByLabel("target_kind").selectOption("fixed_proxy");
  await expect(sheet).toContainText("从代理节点里选");
  const nodeOptions = await sheet.locator('select[name="target_id"] option').allTextContents();
  expect(nodeOptions).toContain("node-eu-1");
  expect(nodeOptions).not.toContain("pool-eu");

  await sheet.getByLabel("target_kind").selectOption("proxy_pool");
  await expect(sheet).toContainText("从代理池里选");
  const poolOptions = await sheet.locator('select[name="target_id"] option').allTextContents();
  expect(poolOptions).toContain("pool-eu");
  expect(poolOptions).not.toContain("node-eu-1");
});

test("a referenced pool is refused before the request, not after", async ({ page }) => {
  await open(page);

  // Scoped: "pool-eu" also appears as a binding's target.
  await page
    .locator(".cp-section", { hasText: "代理池" })
    .locator("tr", { hasText: "pool-eu" })
    .first()
    .getByRole("button", { name: "删除" })
    .click();
  const sheet = page.getByRole("dialog");
  // There is no cascade: the backend refuses. Both lists are already on screen,
  // so the refusal is predicted and its holders named.
  await expect(sheet).toContainText("仍被引用");
  await expect(sheet).toContainText("节点 node-eu-1");
  await expect(sheet).toContainText("绑定 ep-relay-a-responses/cred-relay-key");
});

test("an unreferenced pool deletes, and the panel reflects it", async ({ page }) => {
  await open(page);

  await page
    .locator(".cp-section", { hasText: "代理池" })
    .locator("tr", { hasText: "pool-empty" })
    .getByRole("button", { name: "删除" })
    .click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).not.toContainText("仍被引用");
  await sheet.getByRole("button", { name: "确认删除" }).click();

  await expect(page.locator(".cp-section", { hasText: "代理池" }).locator("tr", { hasText: "pool-empty" })).toHaveCount(0);
});

test("a published version makes every write unavailable", async ({ page }) => {
  await unlock(page);
  await page.locator(".version-picker select").selectOption("v-2026-07");
  await navigate(page, "出口策略");

  const panel = page.locator(PANEL);
  await expect(panel).toContainText("当前版本不是草稿");
  for (const button of await panel.getByRole("button", { name: "新建" }).all()) {
    await expect(button).toBeDisabled();
  }
});

test("ids render in their own case, because ids are case-sensitive", async ({ page }) => {
  await open(page);

  // The shared `th` rule uppercases column labels. These row heads are ids —
  // an operator who retypes NODE-EU-1 from the screen addresses nothing.
  const head = page
    .locator(".cp-section", { hasText: "代理节点" })
    .locator("tr", { hasText: "node-eu-1" })
    .locator("th");
  await expect(head).toHaveText("node-eu-1");
  await expect(head).toHaveCSS("text-transform", "none");
});
