import { expect, test, type Page } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

type DeviceChannel = Readonly<{
  id: "kimi-coding" | "kiro";
  start: string;
  poll: string;
  cancel: string;
  code: string;
  startLabel: string;
  completeLabel: string;
}>;

const DEVICE_CHANNELS: readonly DeviceChannel[] = [
  {
    id: "kimi-coding",
    start: "POST /admin/upstreams/kimi-coding/kimi-authorization/start",
    poll: "POST /admin/upstreams/kimi-coding/kimi-authorization/poll",
    cancel: "POST /admin/upstreams/kimi-coding/kimi-authorization/cancel",
    code: "KIMI-TEST",
    startLabel: "开始授权",
    completeLabel: "Kimi 账号已保存。",
  },
  {
    id: "kiro",
    start: "POST /admin/upstreams/kiro-us-east-1/kiro-authorization/start",
    poll: "POST /admin/upstreams/kiro-us-east-1/kiro-authorization/poll",
    cancel: "POST /admin/upstreams/kiro-us-east-1/kiro-authorization/cancel",
    code: "KIRO-TEST",
    startLabel: "开始授权",
    completeLabel: "账号已保存并连接接口。",
  },
];

async function openDeviceAuthorization(page: Page, channel: DeviceChannel) {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "账号池");
  await page.getByRole("button", { name: "授权 / 导入账号" }).click();
  const chooser = page.getByRole("dialog", { name: "授权或导入账号" });
  await chooser.getByLabel("渠道", { exact: true }).selectOption(channel.id);
  await chooser.getByRole("button", { name: "授权登录", exact: true }).click();
  return page.getByRole("dialog", { name: channel.id === "kiro" ? "授权 Kiro 账号" : "授权 Kimi 账号" });
}

for (const channel of DEVICE_CHANNELS) {
  test(`${channel.id} keeps its device challenge visible during a held poll`, async ({ page }) => {
    const sheet = await openDeviceAuthorization(page, channel);
    await page.evaluate(async ({ poll }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.holdFixtureOperationForTest(poll);
    }, channel);
    await sheet.getByRole("button", { name: channel.startLabel, exact: true }).click();
    await expect(sheet).toContainText(channel.code);
    await expect.poll(async () => page.evaluate(async ({ poll }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      return fixture.fixtureOperationCallsForTest(poll);
    }, channel)).toBe(1);
    await expect(sheet).toContainText(channel.code);
    await expect(sheet.getByRole("link", { name: /打开 .*授权页/u })).toBeVisible();
    await page.evaluate(async ({ poll }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.releaseFixtureOperationForTest(poll);
    }, channel);
    await expect(sheet).toContainText(channel.completeLabel);
  });

  test(`${channel.id} cancels a live device session before returning to the chooser`, async ({ page }) => {
    const sheet = await openDeviceAuthorization(page, channel);
    await page.evaluate(async ({ cancel }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.holdFixtureOperationForTest(cancel);
    }, channel);
    await sheet.getByRole("button", { name: channel.startLabel, exact: true }).click();
    await expect(sheet).toContainText(channel.code);
    await sheet.getByRole("button", { name: "取消授权", exact: true }).click();
    await expect.poll(async () => page.evaluate(async ({ cancel }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      return fixture.fixtureOperationCallsForTest(cancel);
    }, channel)).toBe(1);
    await expect(sheet).toContainText(channel.code);
    await page.evaluate(async ({ cancel }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.releaseFixtureOperationForTest(cancel);
    }, channel);
    await expect(page.getByRole("dialog", { name: "授权或导入账号" })).toBeVisible();
    await expect(sheet).toHaveCount(0);
  });

  test(`${channel.id} exits locally after an unresolved poll without replay or cancellation`, async ({ page }) => {
    const sheet = await openDeviceAuthorization(page, channel);
    await page.evaluate(async ({ id, cancel }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      fixture.failNextDevicePollForTest(id);
      fixture.holdFixtureOperationForTest(cancel);
    }, channel);
    await sheet.getByRole("button", { name: channel.startLabel, exact: true }).click();
    await expect(sheet).toContainText(channel.code);
    await expect(sheet).toContainText("授权结果暂时无法确认");
    await expect(sheet).not.toContainText(channel.code);
    await expect(sheet.getByRole("button", { name: "关闭并标记结果未确认" })).toBeVisible();
    await sheet.getByRole("button", { name: "关闭并标记结果未确认" }).click();
    await expect(page.getByRole("dialog", { name: "授权或导入账号" })).toBeVisible();
    await expect(sheet).toHaveCount(0);
    await expect.poll(async () => page.evaluate(async ({ cancel }) => {
      const fixture = await import("/src/dev/fixtures.ts");
      return fixture.fixtureOperationCallsForTest(cancel);
    }, channel)).toBe(0);
  });
}

test("browser Back cancels a live Kimi session before the router admits departure", async ({ page }) => {
  const channel = DEVICE_CHANNELS[0];
  const sheet = await openDeviceAuthorization(page, channel);
  await page.evaluate(async ({ cancel }) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.holdFixtureOperationForTest(cancel);
  }, channel);
  await sheet.getByRole("button", { name: channel.startLabel, exact: true }).click();
  await expect(sheet).toContainText(channel.code);
  await page.evaluate(() => history.back());
  await expect.poll(async () => page.evaluate(async ({ cancel }) => {
    const fixture = await import("/src/dev/fixtures.ts");
    return fixture.fixtureOperationCallsForTest(cancel);
  }, channel)).toBe(1);
  await expect(page).toHaveURL(/#\/accounts\?add=account$/u);
  await expect(sheet).toContainText(channel.code);
  await page.evaluate(async ({ cancel }) => {
    const fixture = await import("/src/dev/fixtures.ts");
    fixture.releaseFixtureOperationForTest(cancel);
  }, channel);
  await expect(page).toHaveURL(/#\/$/u);
  await expect(sheet).toHaveCount(0);
});
