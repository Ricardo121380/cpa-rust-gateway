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
  await expect(page.locator(".route-workbench")).toBeVisible();
}

async function makeRoute(
  page: import("@playwright/test").Page,
  routeId: string,
): Promise<void> {
  await page.getByRole("button", { name: "新建公开模型" }).click();
  const model = page.getByRole("dialog");
  await model.getByLabel("模型 ID").fill(`pm-${routeId}`);
  await model.getByLabel("模型名", { exact: false }).fill(`m-${routeId}`);
  await model.getByRole("button", { name: "保存" }).click();

  await page
    .locator("tr", { hasText: `m-${routeId}` })
    .first()
    .getByRole("button", { name: "建路由" })
    .click();
  const routeSheet = page.getByRole("dialog");
  await routeSheet.getByLabel("路由 ID").fill(routeId);
  await routeSheet.getByRole("button", { name: "创建" }).click();
}

test("a new route fails validation until a candidate is added", async ({ page }) => {
  await openModels(page);
  await makeRoute(page, "rt-e2e");
  const inventory = page.getByRole("region", { name: "完整配置资源" });
  await expect(inventory).toContainText("rt-e2e");
  await expect(inventory).toContainText("pm-rt-e2e");


  // The panel says what it just did to the draft rather than reporting success.
  await expect(page.locator(".action-notice").first()).toContainText(
    "route_missing_active_candidate",
  );

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
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("exact 模型 ID");
  await sheet.getByLabel("候选 ID").fill("cand-e2e");
  await sheet.getByLabel("endpoint_id", { exact: false }).fill("ep-relay-a-responses");
  await sheet.getByLabel("upstream_model", { exact: false }).fill("relay-x");
  await sheet.getByRole("button", { name: "创建候选" }).click();

  // Adding a candidate re-validates on its own — the operator should not have
  // to re-ask whether the thing they just fixed is fixed.
  await expect(page.locator(".rw-validation")).toHaveAttribute("data-valid", "true");
  await inventory.getByRole("button", { name: "候选", exact: true }).click();
  await expect(inventory).toContainText("cand-e2e");
  await expect(inventory).toContainText("relay-x");
  await inventory.getByRole("button", { name: "编辑候选" }).click();
  const editor = page.getByRole("dialog");
  await expect(editor.getByLabel("候选 ID")).toHaveValue("cand-e2e");
  await expect(editor.getByLabel("候选 ID")).toHaveAttribute("readonly", "");
  await editor.getByLabel("weight", { exact: false }).fill("7");
  await editor.getByLabel("priority", { exact: false }).fill("3");
  await editor.getByLabel("transform_mode", { exact: false }).selectOption("canonical_bridge");
  await editor.getByLabel("capability_override", { exact: false }).fill("vision=false tools=true");
  await editor.getByRole("button", { name: "保存候选" }).click();
  await expect(editor).not.toBeVisible();
  await expect(inventory).toContainText("权重 7");
  await inventory.getByRole("button", { name: "编辑候选" }).click();
  await expect(editor.getByLabel("priority", { exact: false })).toHaveValue("3");
  await expect(editor.getByLabel("transform_mode", { exact: false })).toHaveValue("canonical_bridge");
  await expect(editor.getByLabel("capability_override", { exact: false })).toHaveValue("vision=false tools=true");
  await editor.getByRole("button", { name: "取消", exact: true }).click();
  await inventory.getByRole("button", { name: "删除候选" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认删除候选" }).click();
  await expect(inventory).not.toContainText("cand-e2e");
  await expect(page.locator(".rw-validation")).toHaveAttribute("data-valid", "false");
  await inventory.getByRole("button", { name: "路由", exact: true }).click();
  await expect(inventory).toContainText("rt-e2e");

  await inventory.getByRole("button", { name: "别名", exact: true }).click();
  await expect(inventory).toContainText("此版本暂无该类资源");

});

test("capability_override rejects a non-boolean instead of coercing it", async ({ page }) => {
  await openModels(page);
  await makeRoute(page, "rt-e2e-cap");
  await page.locator(".route-workbench").getByRole("button", { name: "打开它" }).click();
  await page.locator(".route-workbench").getByRole("button", { name: "加候选" }).click();

  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("候选 ID").fill("cand-cap");
  await sheet.getByLabel("endpoint_id", { exact: false }).fill("ep-relay-a-responses");
  await sheet.getByLabel("upstream_model", { exact: false }).fill("relay-x");
  await sheet.getByLabel("capability_override", { exact: false }).fill("vision=1");
  await sheet.getByRole("button", { name: "创建候选" }).click();

  await expect(page.locator(".action-error")).toContainText("只能是 true 或 false");
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
  await navigate(page, "运行诊断");
  await page.getByRole("form", { name: "路由解释" }).getByLabel("route_id", { exact: true }).fill("rt-minimax");
  await page.getByLabel("请求模型").fill("minimax-m3");
  await page.getByRole("button", { name: "解释" }).click();

  const explainCard = page.locator(".rt-card", { hasText: "路由解释" });
  await expect(explainCard).toContainText("草稿版本没有可解释的快照");
  await expect(explainCard).toContainText("快照只在版本发布后存在");
});

test("route explain shows price evidence and the catalog it came from", async ({ page }) => {
  await unlock(page);
  await selectActive(page);
  await navigate(page, "运行诊断");

  await page.getByRole("form", { name: "路由解释" }).getByLabel("route_id", { exact: true }).fill("rt-minimax");
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
  await navigate(page, "运行诊断");

  await page.getByRole("form", { name: "路由解释" }).getByLabel("route_id", { exact: true }).fill("rt-multi-provider");
  await page.getByLabel("请求模型").fill("minimax-m3");
  await page.getByRole("button", { name: "解释" }).click();

  // Not "Management operation failed": the panel names the fix.
  const explainCard = page.locator(".rt-card", { hasText: "路由解释" });
  await expect(explainCard).toContainText("需要显式指定 Provider");
  await expect(explainCard).toContainText("必须显式选一个");

  await page.getByRole("form", { name: "路由解释" }).getByLabel("provider_id", { exact: false }).fill("prov-a");
  await page.getByRole("button", { name: "解释" }).click();
  await expect(page.locator('.rt-chip[data-state="dominant"]')).toBeVisible();
});

test("explain offers all three contract protocols", async ({ page }) => {
  await unlock(page);
  await selectActive(page);
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
