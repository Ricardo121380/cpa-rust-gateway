import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

// The subresource half of the configuration chain. Before this, the panel
// could show channels/accounts/bindings but not create one — everything in a
// working config had to be made outside Prism.

async function openPanel(
  page: import("@playwright/test").Page,
  prepare?: (page: import("@playwright/test").Page) => Promise<void>,
): Promise<void> {
  await unlock(page);
  await selectDraft(page);
  await prepare?.(page);
  await navigate(page, "上游");
  await page.locator("article", { hasText: "中转站 A" }).getByRole("button", { name: "接口与账号" }).click();
  await expect(page.locator(".subresource-panel")).toBeVisible();
}

async function openPanelFromActiveConfig(page: import("@playwright/test").Page): Promise<void> {
  await unlock(page);
  // The fixture's pre-seeded provider graph begins in the editable draft.
  // Publish it through the same control-plane workflow to exercise a provider
  // write from an actual active configuration, rather than fabricating one.
  await selectDraft(page);
  await page.locator(".dock").getByRole("button", { name: "发布", exact: true }).click();
  await page.getByRole("dialog", { name: "确认发布" }).getByRole("button", { name: "确认发布", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("已发布");
  await page.getByRole("button", { name: "完成", exact: true }).click();
  await navigate(page, "上游");
  await page.locator("article", { hasText: "中转站 A" }).getByRole("button", { name: "接口与账号" }).click();
  await expect(page.locator(".subresource-panel")).toBeVisible();
}

async function createEndpoint(page: import("@playwright/test").Page, id: string): Promise<void> {
  await page.getByRole("button", { name: "新建接口" }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("接口标识", { exact: true }).fill(id);
  await sheet.getByLabel("接口实现").fill("openai-compatible.responses");
  await sheet.getByLabel("请求协议").fill("openai/responses");
  await sheet.getByLabel("接口地址").fill(`https://${id}.example/v1`);
  await sheet.getByLabel("请求路径").fill("/responses");
  await sheet.getByRole("button", { name: "创建接口" }).click();
}

async function interceptPublication(
  page: import("@playwright/test").Page,
  mode: "before" | "after",
): Promise<void> {
  await page.evaluate(async (failureMode) => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__providerPublicationCalls", 0);
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation !== "publishConfigVersion") return original.call(this, operation, request);
      Reflect.set(globalThis, "__providerPublicationCalls", Number(Reflect.get(globalThis, "__providerPublicationCalls")) + 1);
      if (failureMode === "before") throw new Error("simulated publication transport failure");
      await original.call(this, operation, request);
      throw new Error("simulated lost publication response");
    };
  }, mode);
}

async function publicationCalls(page: import("@playwright/test").Page): Promise<number> {
  return page.evaluate(() => Number(Reflect.get(globalThis, "__providerPublicationCalls")));
}

async function loseEndpointWriteResponse(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__providerEndpointWriteCalls", 0);
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation !== "createEndpoint") return original.call(this, operation, request);
      Reflect.set(globalThis, "__providerEndpointWriteCalls", Number(Reflect.get(globalThis, "__providerEndpointWriteCalls")) + 1);
      await original.call(this, operation, request);
      throw new Error("simulated lost endpoint response");
    };
  });
}

async function endpointWriteCalls(page: import("@playwright/test").Page): Promise<number> {
  return page.evaluate(() => Number(Reflect.get(globalThis, "__providerEndpointWriteCalls")));
}

async function removeEndpointRevisionReceipt(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__providerEndpointWriteCalls", 0);
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      const response = await original.call(this, operation, request);
      if (operation !== "createEndpoint") return response;
      Reflect.set(globalThis, "__providerEndpointWriteCalls", Number(Reflect.get(globalThis, "__providerEndpointWriteCalls")) + 1);
      const body = await response.text();
      return new Response(body, { status: response.status, headers: { "Content-Type": "application/json" } });
    };
  });
}

async function loseEndpointDeleteResponse(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__providerEndpointDeleteCalls", 0);
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation !== "deleteEndpoint") return original.call(this, operation, request);
      Reflect.set(globalThis, "__providerEndpointDeleteCalls", Number(Reflect.get(globalThis, "__providerEndpointDeleteCalls")) + 1);
      await original.call(this, operation, request);
      throw new Error("simulated lost endpoint deletion response");
    };
  });
}

async function endpointDeleteCalls(page: import("@playwright/test").Page): Promise<number> {
  return page.evaluate(() => Number(Reflect.get(globalThis, "__providerEndpointDeleteCalls")));
}

async function changeEndpointBeforeDelete(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(async () => {
    const { ManagementApi } = await import("/src/generated/management-client.ts");
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, "__providerEndpointDeleteCalls", 0);
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation === "deleteEndpoint") {
        Reflect.set(globalThis, "__providerEndpointDeleteCalls", Number(Reflect.get(globalThis, "__providerEndpointDeleteCalls")) + 1);
        return original.call(this, operation, request);
      }
      const response = await original.call(this, operation, request);
      if (operation !== "getEndpoint") return response;
      const body = await response.json() as Record<string, unknown>;
      return new Response(JSON.stringify({ ...body, inference_path: "/changed" }), { status: response.status, headers: response.headers });
    };
  });
}

test("runtime observation failures do not claim an interface has no saved account", async ({ page }) => {
  await openPanel(page, async (current) => {
    await current.evaluate(async () => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.failNextOperationalPoolForTest();
    });
  });
  const endpoint = page.locator('[data-resource-id="ep-relay-a-responses"]').first();
  await expect(endpoint).toContainText("运行连接暂不可读取");
  await expect(endpoint).not.toContainText("尚未连接账号");
});

test("partial runtime observations never claim a complete connection count", async ({ page }) => {
  await openPanel(page, async (current) => {
    await current.evaluate(async () => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.usePartialOperationalPoolForTest();
    });
  });
  const endpoint = page.locator('[data-resource-id="ep-relay-a-responses"]').first();
  await expect(endpoint).toContainText("已加载的运行调度中未观测到账号");
  await expect(endpoint).not.toContainText("尚未连接账号");
});

test("ordinary provider rows fit the active detail pane without horizontal scrolling", async ({ page }) => {
  for (const viewport of [{ width: 1280, height: 900 }, { width: 1440, height: 900 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    await openPanel(page);
    const fits = await page.locator('.subresource-row').first().evaluate((row) => {
      const pane = row.closest<HTMLElement>(".provider-detail");
      if (pane === null) return false;
      const paneBounds = pane.getBoundingClientRect();
      const rowBounds = row.getBoundingClientRect();
      return pane.scrollWidth <= pane.clientWidth && rowBounds.left >= paneBounds.left && rowBounds.right <= paneBounds.right;
    });
    expect(fits).toBe(true);
    await page.goto("/");
  }
});

test("a channel can be created and appears in the inventory", async ({ page }) => {
  await openPanel(page);
  await createEndpoint(page, "ep-e2e");
  await page.getByRole("button", { name: "查看工作配置" }).click();

  // Complete configuration inventory exposes the new endpoint before any binding exists.
  await expect(page.locator('.subresource-panel [data-resource-id="ep-e2e"]').first()).toBeVisible();

  await page.locator(".subresource-panel").getByRole("button", { name: "连接账号" }).first().click();
  const bind = page.getByRole("dialog");
  await expect(bind.getByRole("combobox", { name: "接口" })).toHaveValue("");
  await bind.getByRole("combobox", { name: "接口" }).selectOption("ep-e2e");
  await bind.getByRole("combobox", { name: "账号" }).selectOption("cred-relay-key");
  await expect(bind.getByLabel("权重")).toHaveAttribute("min", "1");
  await expect(bind.getByLabel("权重")).toHaveAttribute("max", "10000");
  await expect(bind.getByLabel("并发上限")).toHaveAttribute("min", "1");
  await expect(bind.getByRole("button", { name: "保存连接" })).toBeVisible();
  await bind.getByRole("button", { name: "保存连接" }).click();
  await page.getByRole("button", { name: "查看工作配置" }).click();

  // now it exists in the inventory
  await expect(page.locator('.subresource-panel [data-resource-id="ep-e2e"]').first()).toBeVisible();
  const scheduling = page.locator("details", { hasText: "高级调度配置" });
  await scheduling.locator("summary").click();
  await expect(scheduling.locator('tr[data-endpoint-id="ep-e2e"][data-account-id="cred-relay-key"]')).toBeVisible();
});

test("a pending provider receipt keeps its originating workspace selected", async ({ page }) => {
  await openPanel(page);
  await createEndpoint(page, "ep-provider-origin");
  await expect(page.locator('.subresource-panel[role="status"]')).toContainText("修改已保存到草稿");

  const otherProvider = page.locator("article", { hasText: "Grok Build 池" });
  await expect(otherProvider.getByRole("button", { name: "接口与账号", exact: true })).toBeDisabled();
  await expect(page.locator(".provider-detail > header")).toContainText("中转站 A");
});

test("an unapplied provider receipt cannot be relabelled under another workspace", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await interceptPublication(page, "before");
  await createEndpoint(page, "ep-provider-unapplied");
  await expect(page.locator('.subresource-panel[role="status"]')).toContainText("修改需要核对");
  const otherProvider = page.locator("article", { hasText: "Grok Build 池" });
  await expect(otherProvider.getByRole("button", { name: "接口与账号", exact: true })).toBeDisabled();
});

test("editing a channel pre-fills the URL the inventory does not carry", async ({ page }) => {
  await openPanel(page);
  await page
    .locator('[data-resource-id="ep-relay-a-responses"]')
    .first()
    .getByRole("button", { name: "编辑" })
    .click();
  const sheet = page.getByRole("dialog");
  // base_url comes from getEndpoint — account-pools is URL-free by contract,
  // and a blank field here would erase it on save.
  await expect(sheet.getByLabel("接口地址")).toHaveValue("https://relay-a.example.com/v1");
  await expect(sheet.getByLabel("请求路径")).toHaveValue("/responses");
  await sheet.getByLabel("请求协议").fill("openai/chat-completions");
  await sheet.getByRole("button", { name: "保存修改" }).click();
  await page.getByRole("button", { name: "查看工作配置" }).click();
  await expect(page.locator(".subresource-panel")).toContainText("Chat Completions");
});

test("editing an account demands the secret again, and says why", async ({ page }) => {
  await openPanel(page);
  await page
    .locator(".subresource-panel")
    .locator('[data-resource-id="cred-relay-key"]')
    .getByRole("button", { name: "高级编辑凭据" })
    .click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toContainText("更新授权资料会重新校验账号");
  await expect(sheet.getByLabel("授权资料")).toHaveAttribute("required", "");
  // and it is not a password field — those swallow paste in Safari
  await expect(sheet.getByLabel("授权资料")).toHaveAttribute("type", "text");
});

test("deleting a channel warns that its bindings and candidates go with it", async ({ page }) => {
  await openPanel(page);
  await page
    .locator('[data-resource-id="ep-relay-a-responses"]')
    .first()
    .getByRole("button", { name: "删除" })
    .click();
  const confirm = page.getByRole("dialog");
  await expect(confirm).toContainText("连带移除它的全部绑定");
  await expect(confirm).toContainText("路由候选将失去目标");
  await confirm.getByRole("button", { name: "确认删除" }).click();
  await page.getByRole("button", { name: "查看工作配置" }).click();
  // Removing its usage locations must not hide the retained credentials.
  await expect(page.locator('.subresource-list').first().locator('[data-resource-id="ep-relay-a-responses"]')).toHaveCount(0);
  await expect(page.locator('.subresource-panel [data-resource-id="cred-relay-key"]').first()).toBeVisible();
});

test("a lost endpoint deletion response becomes a review receipt without a second DELETE", async ({ page }) => {
  await openPanel(page);
  await loseEndpointDeleteResponse(page);
  await page.locator('[data-resource-id="ep-relay-a-responses"]').first().getByRole("button", { name: "删除" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "确认删除" }).click();
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改结果未确认");
  expect(await endpointDeleteCalls(page)).toBe(1);
  await receipt.getByRole("button", { name: "查看工作配置", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-relay-a-responses"]')).toHaveCount(0);
});

test("a changed endpoint is rejected before its captured deletion can be sent", async ({ page }) => {
  await openPanel(page);
  await changeEndpointBeforeDelete(page);
  await page.locator('[data-resource-id="ep-relay-a-responses"]').first().getByRole("button", { name: "删除" }).click();
  const confirm = page.getByRole("dialog");
  await confirm.getByRole("button", { name: "确认删除" }).click();
  await expect(confirm).toContainText("接口已变化");
  expect(await endpointDeleteCalls(page)).toBe(0);
});

test("an active configuration applies a provider resource write and retains the workspace", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await createEndpoint(page, "ep-active-receipt");
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改已应用");
  await expect(receipt).toContainText("已保存并应用到运行服务");
  await receipt.getByRole("button", { name: "完成", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-active-receipt"]')).toBeVisible();
});

test("a publication failure after a provider write gives one reviewable draft receipt without replay", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await interceptPublication(page, "before");
  await createEndpoint(page, "ep-apply-review");
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改需要核对");
  await expect(receipt).toContainText("已保存，但应用尚未完成");
  expect(await publicationCalls(page)).toBe(1);
  await receipt.getByRole("button", { name: "查看工作配置", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-apply-review"]')).toBeVisible();
});

test("a lost publication response reconciles a durable active result without replay", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await interceptPublication(page, "after");
  await createEndpoint(page, "ep-apply-reread");
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改已应用");
  await expect(receipt).toContainText("应用回执需要重新核对");
  expect(await publicationCalls(page)).toBe(1);
  await receipt.getByRole("button", { name: "完成", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-apply-reread"]')).toBeVisible();
});

test("a lost endpoint response becomes a non-replayable review receipt", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await loseEndpointWriteResponse(page);
  await createEndpoint(page, "ep-write-uncertain");
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改结果未确认");
  await expect(receipt).toContainText("不要重复提交本次操作");
  expect(await endpointWriteCalls(page)).toBe(1);
  await receipt.getByRole("button", { name: "查看工作配置", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-write-uncertain"]')).toBeVisible();
});

test("an unconfirmed provider receipt cannot be relabelled under another workspace", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await loseEndpointWriteResponse(page);
  await createEndpoint(page, "ep-provider-unconfirmed");
  await expect(page.locator('.subresource-panel[role="status"]')).toContainText("修改结果未确认");
  const otherProvider = page.locator("article", { hasText: "Grok Build 池" });
  await expect(otherProvider.getByRole("button", { name: "接口与账号", exact: true })).toBeDisabled();
});

test("a missing endpoint revision receipt becomes a non-replayable review receipt", async ({ page }) => {
  await openPanelFromActiveConfig(page);
  await removeEndpointRevisionReceipt(page);
  await createEndpoint(page, "ep-write-no-revision");
  const receipt = page.locator('.subresource-panel[role="status"]');
  await expect(receipt).toContainText("修改结果未确认");
  expect(await endpointWriteCalls(page)).toBe(1);
  await receipt.getByRole("button", { name: "查看工作配置", exact: true }).click();
  await expect(page.locator('[data-resource-id="ep-write-no-revision"]')).toBeVisible();
});


test("new account opens the channel-owned authorization and import chooser", async ({ page }) => {
  await openPanel(page);
  await page.getByRole("button", { name: "添加账号", exact: true }).click();
  const sheet = page.getByRole("dialog");
  await expect(sheet).toHaveAccessibleName("授权或导入账号");
  await expect(sheet.getByLabel("渠道")).toBeVisible();
  await expect(sheet.getByLabel("认证方式")).toHaveCount(0);
});


test("new unbound credentials remain visible and editable", async ({ page }) => {
  await openPanel(page);
  await page.locator("details", { hasText: "高级凭据维护" }).locator("summary").click();
  await page.getByRole("button", { name: "添加原始凭据", exact: true }).click();
  const sheet = page.getByRole("dialog");
  await sheet.getByLabel("凭据标识", { exact: true }).fill("account-unbound");
  await sheet.getByLabel("授权资料", { exact: true }).fill("synthetic-test-token");
  await sheet.getByRole("button", { name: "保存原始凭据", exact: true }).click();
  await page.getByRole("button", { name: "查看工作配置" }).click();
  const row = page.locator(".subresource-list").last().locator('[data-resource-id="account-unbound"]');
  await expect(row).toBeVisible();
  await row.getByRole("button", { name: "高级编辑凭据", exact: true }).click();
  await expect(page.getByRole("dialog").locator('input[name="id"]')).toHaveValue("account-unbound");
});
