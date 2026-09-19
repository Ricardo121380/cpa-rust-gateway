import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

async function expectFooterForm(
  dialog: import("@playwright/test").Locator,
  formId: string,
  action: string,
): Promise<void> {
  const footer = dialog.locator(".sheet-footer");
  await expect(footer).toBeVisible();
  await expect(footer.getByRole("button", { name: action })).toHaveAttribute("form", formId);
}

/**
 * Exercise the browser's real indexed history rather than changing a hash by
 * hand. React Router must observe the departure and its own compensating POP
 * before the held write is released; otherwise this test can pass before a
 * navigation has actually been processed.
 */
async function expectBusyHistoryRoundTrip(page: import("@playwright/test").Page, delta: number) {
  return page.evaluate(async (historyDelta) => {
    const protectedHref = location.href;
    const protectedState = structuredClone(history.state);
    return new Promise<{ events: number; state: unknown }>((resolve, reject) => {
      let leftProtectedEntry = false;
      let events = 0;
      const timeout = window.setTimeout(() => {
        window.removeEventListener("popstate", observe);
        reject(new Error(`history.go(${historyDelta}) did not leave and restore the protected entry`));
      }, 2_000);
      function observe() {
        events += 1;
        if (location.href !== protectedHref) leftProtectedEntry = true;
        if (leftProtectedEntry && location.href === protectedHref) {
          window.clearTimeout(timeout);
          window.removeEventListener("popstate", observe);
          resolve({ events, state: structuredClone(history.state) });
        }
      }
      window.addEventListener("popstate", observe);
      history.go(historyDelta);
    });
  }, delta);
}

test("daily forms keep their submit actions in stable, native form-associated footers", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);

  await navigate(page, "账号池");
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();
  const account = page.getByRole("dialog", { name: "授权或导入账号" });
  await expectFooterForm(account, "account-import-form", "导入账号");
  await expect(account.locator("#account-import-form")).toBeVisible();
  await account.getByRole("button", { name: "导入账号" }).click();
  await expect(account).toBeVisible();
  await account.getByRole("button", { name: "取消" }).click();

  await navigate(page, "模型与路由");
  await page.getByRole("button", { name: "接入模型" }).click();
  const model = page.getByRole("dialog", { name: "接入模型" });
  await expectFooterForm(model, "connect-model-form", "保存并应用");
  await expect(model.locator("#connect-model-form")).toBeVisible();
  await model.getByRole("button", { name: "取消" }).click();

  await navigate(page, "上游");
  await page.getByRole("button", { name: "添加提供商" }).click();
  const provider = page.getByRole("dialog", { name: "添加 AI 提供商" });
  await expectFooterForm(provider, "provider-setup-form", "保存并应用");
  await expect(provider.locator("#provider-setup-form")).toBeVisible();
  await provider.getByRole("button", { name: "取消" }).click();

  await navigate(page, "访问控制");
  await page.getByRole("button", { name: "创建客户端密钥" }).click();
  const key = page.getByRole("dialog", { name: "创建 API 密钥" });
  await expectFooterForm(key, "issue-key-form", "创建并应用");
  await expect(key.locator("#issue-key-form")).toBeVisible();
});

test("busy and dirty sheets preserve indexed Back and Forward history", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async () => {
    const { call } = await import("/src/api/client.ts");
    await call(
      "createRoute",
      {
        path: { public_model_id: "pm-minimax" },
        body: { id: "rt-modal-receipt", policy: "smooth_weighted_round_robin", max_attempts: 1, bootstrap_timeout_ms: 1_000 },
      },
      { versionScoped: true, mutating: true },
    );
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest("POST /admin/client-keys");
  });

  // Build real HashRouter entries on both sides of Access. The protected
  // entry is deliberately in the middle of the stack so both multi-step Back
  // and multi-step Forward exercise React Router's POP restoration.
  await navigate(page, "账号池");
  await navigate(page, "访问控制");
  await navigate(page, "模型与路由");
  await navigate(page, "上游");
  await page.evaluate(() => history.go(-2));
  await expect(page).toHaveURL(/#\/access$/u);

  await page.getByRole("button", { name: "创建客户端密钥" }).click();
  const sheet = page.getByRole("dialog", { name: "创建 API 密钥" });
  await sheet.getByLabel("名称").fill("pending receipt");
  await sheet.getByRole("checkbox", { name: "minimax-m3" }).check();
  await sheet.locator(".sheet-footer").getByRole("button", { name: "创建并应用" }).click();

  await expect.poll(async () => page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest("POST /admin/client-keys");
  })).toBe(1);
  await expect(sheet.getByRole("button", { name: "关闭面板" })).toBeDisabled();
  await page.keyboard.press("Escape");
  const sourceState = await page.evaluate(() => structuredClone(history.state));
  const rejectedBack = await expectBusyHistoryRoundTrip(page, -2);
  expect(rejectedBack.events).toBeGreaterThanOrEqual(2);
  expect(rejectedBack.state).toEqual(sourceState);
  await expect(page).toHaveURL(/#\/access$/u);
  await expect(sheet).toBeVisible();

  const rejectedForward = await expectBusyHistoryRoundTrip(page, 2);
  expect(rejectedForward.events).toBeGreaterThanOrEqual(2);
  expect(rejectedForward.state).toEqual(sourceState);
  await expect(page).toHaveURL(/#\/access$/u);
  await expect(sheet).toBeVisible();

  await page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest("POST /admin/client-keys");
  });
  await expect(page.getByRole("dialog", { name: "API 密钥已生成" }).locator(".reveal-key")).toBeVisible();
  await expect(page).toHaveURL(/#\/access$/u);
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await page.getByRole("dialog", { name: "API 密钥已生成" }).getByRole("button", { name: "核对配置并清除密钥" }).click();
  await expect.poll(async () => page.evaluate(async () => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest("POST /admin/client-keys");
  })).toBe(1);

  // Completion removes the busy guard. Navigate away, return through the real
  // browser history, then prove a regular dirty form can Keep editing or
  // Discard after a blocked Back without a reload or route/screen mismatch.
  await navigate(page, "账号池");
  await expect(page).toHaveURL(/#\/accounts$/u);
  await page.evaluate(() => history.back());
  await expect(page).toHaveURL(/#\/access$/u);
  await page.getByRole("button", { name: "创建客户端密钥" }).click();
  const dirtySheet = page.getByRole("dialog", { name: "创建 API 密钥" });
  await dirtySheet.getByLabel("名称").fill("keep or discard");

  await page.evaluate(() => history.back());
  const discard = page.getByRole("alertdialog", { name: "放弃未保存的修改？" });
  await expect(discard).toBeVisible();
  await discard.getByRole("button", { name: "继续编辑" }).click();
  await expect(page).toHaveURL(/#\/access$/u);
  await expect(dirtySheet).toBeVisible();

  await page.evaluate(() => history.back());
  await expect(discard).toBeVisible();
  await discard.getByRole("button", { name: "放弃修改" }).click();
  await expect(page).toHaveURL(/#\/accounts$/u);
  await expect(dirtySheet).toHaveCount(0);
});
