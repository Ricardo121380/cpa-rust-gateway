import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

const STATUS = "GET /admin/credentials/cred-codex-oauth/oauth/status";
const CANCEL = "POST /admin/credentials/cred-codex-oauth/oauth/cancel";

async function openLegacyRenewal(page: Page) {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await page.locator("article", { hasText: "中转站 A" }).getByRole("button", { name: "接口与账号" }).click();
  await page.locator('[data-resource-id="cred-codex-oauth"]').first().getByRole("button", { name: "详情" }).click();
  const detail = page.getByRole("dialog", { name: "账号详情" });
  await detail.getByRole("button", { name: "配置" }).click();
  await detail.getByRole("button", { name: "重新授权" }).click();
  return page.getByRole("dialog", { name: "重新授权" });
}

test("legacy renewal keeps its official link while a compact status read is held", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, STATUS);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  const link = wizard.getByRole("link", { name: "打开官方授权页" });
  const href = await link.getAttribute("href");
  expect(href).toBeTruthy();
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, STATUS)).toBe(1);
  await expect(link).toHaveAttribute("href", href ?? "");
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(route);
  }, STATUS);
  await expect(link).toHaveAttribute("href", href ?? "");
  await expect(wizard).toContainText("等待官方授权");
});

test("legacy renewal completes exactly once through its native footer form", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  const link = wizard.getByRole("link", { name: "打开官方授权页" });
  const authorize = await link.getAttribute("href");
  const state = new URL(authorize ?? "").searchParams.get("state");
  expect(state).toBeTruthy();
  const submit = wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" });
  const formId = await submit.getAttribute("form");
  expect(formId).toMatch(/^credential-oauth-callback-/u);
  await expect(wizard.locator(`#${formId}`)).toBeVisible();
  await wizard.getByLabel("回调地址").fill(`http://127.0.0.1:8085/callback?code=fixture-code&state=${state ?? ""}`);
  await submit.click();
  await expect(wizard).toContainText("本次授权回调已由服务端确认并保存");
  await expect(wizard.getByLabel("回调地址")).toHaveCount(0);
  await wizard.getByRole("button", { name: "完成", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "账号详情" })).toBeVisible();
});

test("a mismatched callback stays recoverable and does not consume the live session", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await wizard.getByLabel("回调地址").fill("http://127.0.0.1:8085/callback?code=wrong&state=st-other-session");
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect(wizard).toContainText("原会话仍在等待");
  await expect(wizard).toContainText("等待官方授权");
  await expect(wizard.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
  await expect(wizard.getByLabel("回调地址")).toHaveValue("");
  await expect(wizard.getByRole("button", { name: "完成授权" })).toBeVisible();
});

test("a production-shaped rejection stays unresolved when status cannot be reconciled", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  const callbackRoute = "POST /admin/credentials/cred-codex-oauth/oauth/callback";
  await wizard.getByRole("button", { name: "启动授权" }).click();
  const state = new URL(await wizard.getByRole("link", { name: "打开官方授权页" }).getAttribute("href") ?? "").searchParams.get("state");
  await page.evaluate(async ({ callback, cancel }) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.trackFixtureOperationForTest(callback);
    fixture.trackFixtureOperationForTest(cancel);
    fixture.failNextCredentialOAuthStatusForTest();
  }, { callback: callbackRoute, cancel: CANCEL });
  await wizard.getByLabel("回调地址").fill("?code=wrong&state=st-other-session");
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect(wizard).toContainText("授权结果暂时无法确认");
  await expect(wizard.getByRole("button", { name: "完成授权" })).toHaveCount(0);
  await expect(wizard.getByRole("button", { name: "关闭并标记结果未确认" })).toBeVisible();
  await wizard.getByRole("button", { name: "重新读取状态" }).click();
  await expect(wizard.getByRole("button", { name: "完成授权" })).toBeVisible();
  await wizard.getByLabel("回调地址").fill(`?code=fixture-code&state=${state ?? ""}`);
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect(wizard).toContainText("本次授权回调已由服务端确认并保存");
  await expect.poll(async () => page.evaluate(async ({ callback, cancel }) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return [fixture.fixtureOperationCallsForTest(callback), fixture.fixtureOperationCallsForTest(cancel)];
  }, { callback: callbackRoute, cancel: CANCEL })).toEqual([2, 0]);
});

test("an invalid callback never reaches the gateway", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  const callbackRoute = "POST /admin/credentials/cred-codex-oauth/oauth/callback";
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, callbackRoute);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await wizard.getByLabel("回调地址").fill("?code=without-state");
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect(wizard).toContainText("没有 state 参数");
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, callbackRoute)).toBe(0);
});

test("a direct callback acknowledgement remains a receipt when its reread fails", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  const state = new URL(await wizard.getByRole("link", { name: "打开官方授权页" }).getAttribute("href") ?? "").searchParams.get("state");
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.failNextCredentialReadForTest();
  });
  await wizard.getByLabel("回调地址").fill(`?code=fixture-code&state=${state ?? ""}`);
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect(wizard).toContainText("本次授权回调已由服务端确认并保存");
  await expect(wizard).toContainText("账号资料将稍后重新读取");
  await expect(wizard.getByRole("button", { name: "完成", exact: true })).toBeVisible();
});

test("cancelling a legacy renewal waits for the server before closing", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, CANCEL);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await wizard.getByRole("button", { name: "取消授权" }).click();
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, CANCEL)).toBe(1);
  await expect(wizard).toBeVisible();
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(route);
  }, CANCEL);
  await expect(wizard).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "账号详情" })).toBeVisible();
});

test("browser Back cancels a live legacy renewal before the router admits departure", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, CANCEL);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await expect(wizard.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
  const protectedUrl = page.url();
  await page.evaluate(() => history.back());
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, CANCEL)).toBe(1);
  await expect(page).toHaveURL(protectedUrl);
  await expect(wizard).toBeVisible();
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(route);
  }, CANCEL);
  await expect(page).not.toHaveURL(protectedUrl);
  await expect(wizard).toHaveCount(0);
});

for (const terminal of ["expired", "complete"] as const) {
  test(`a reopened renewal does not reuse a cached ${terminal} status`, async ({ page }) => {
    const first = await openLegacyRenewal(page);
    await first.getByRole("button", { name: "启动授权" }).click();
    await expect(first.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
    await page.evaluate(async ({ credentialId, terminalState }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.forceCredentialOAuthTerminalForTest(credentialId, terminalState);
    }, { credentialId: "cred-codex-oauth", terminalState: terminal });
    await expect(first).toContainText(terminal === "expired" ? "授权已过期" : "服务端报告账号已保存");
    await first.getByRole("button", { name: terminal === "complete" ? "关闭并标记结果未确认" : "关闭", exact: true }).click();
    const detail = page.getByRole("dialog", { name: "账号详情" });
    await expect(detail).toBeVisible();
    await detail.getByRole("button", { name: "配置" }).click();
    await page.evaluate(async (route) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.holdFixtureOperationForTest(route);
    }, STATUS);
    await detail.getByRole("button", { name: "重新授权" }).click();
    const reopened = page.getByRole("dialog", { name: "重新授权" });
    await reopened.getByRole("button", { name: "启动授权" }).click();
    await expect(reopened.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
    await expect.poll(async () => page.evaluate(async (route) => {
      const fixture = await import("/src/dev/fixtures.ts");
      return fixture.fixtureOperationCallsForTest(route);
    }, STATUS)).toBe(1);
    await expect(reopened).not.toContainText(terminal === "expired" ? "授权已过期" : "服务端报告账号已保存");
    await page.evaluate(async (route) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.releaseFixtureOperationForTest(route);
    }, STATUS);
  });
}

test("a lost callback response stays unreplayable after durable persistence", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  const callbackRoute = "POST /admin/credentials/cred-codex-oauth/oauth/callback";
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.loseNextCredentialOAuthCompletionResponseForTest();
    fixture.holdFixtureOperationForTest(route);
  }, callbackRoute);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  const state = new URL(await wizard.getByRole("link", { name: "打开官方授权页" }).getAttribute("href") ?? "").searchParams.get("state");
  await wizard.getByLabel("回调地址").fill(`?code=fixture-code&state=${state ?? ""}`);
  await wizard.locator(".sheet-footer").getByRole("button", { name: "完成授权" }).click();
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, callbackRoute)).toBe(1);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(route);
  }, callbackRoute);
  await expect(wizard).toContainText("授权结果暂时无法确认");
  await expect(wizard.getByRole("button", { name: "完成授权" })).toHaveCount(0);
  await wizard.getByRole("button", { name: "重新读取状态" }).click();
  await expect(wizard).toContainText("无法确认");
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, callbackRoute)).toBe(1);
});

test("an acknowledged cancellation never retries when status reconciliation fails", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, CANCEL);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await expect(wizard.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.failNextCredentialOAuthStatusForTest();
  });
  await wizard.getByRole("button", { name: "取消授权" }).click();
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, CANCEL)).toBe(1);
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(route);
  }, CANCEL);
  await expect(wizard).toContainText("取消请求已确认");
  await expect(wizard.getByRole("button", { name: "取消授权" })).toHaveCount(0);
  await wizard.getByRole("button", { name: "关闭并标记结果未确认" }).click();
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, CANCEL)).toBe(1);
});

test("an acknowledged cancellation with pending status has recovery instead of a stuck footer", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  const callbackRoute = "POST /admin/credentials/cred-codex-oauth/oauth/callback";
  await page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(route);
  }, callbackRoute);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await expect(wizard.getByRole("link", { name: "打开官方授权页" })).toBeVisible();
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.retainPendingCredentialOAuthOnCancelForTest();
  });
  await wizard.getByRole("button", { name: "取消授权" }).click();
  await expect(wizard).toContainText("取消请求已确认，但服务端仍在处理授权结果");
  await expect(wizard.getByRole("button", { name: "重新读取状态" })).toBeVisible();
  await expect(wizard.getByRole("button", { name: "关闭并标记结果未确认" })).toBeVisible();
  await expect(wizard.getByRole("button", { name: "正在确认…" })).toHaveCount(0);
  await expect.poll(async () => page.evaluate(async (route) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(route);
  }, callbackRoute)).toBe(0);
});

test("a durable-account status fallback is never presented as this renewal succeeding", async ({ page }) => {
  const wizard = await openLegacyRenewal(page);
  await wizard.getByRole("button", { name: "启动授权" }).click();
  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.dropCredentialOAuthSessionForTest();
  });
  await expect(wizard).toContainText("服务端报告账号已保存");
  await expect(wizard).toContainText("无法确认本次续授权是否完成");
  await expect(wizard).not.toContainText("本次授权回调已由服务端确认并保存");
});
