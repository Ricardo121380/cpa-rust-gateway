import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("a failed config read labels prior results and can be retried without losing the session", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "上游");
  await expect(page.locator("tbody")).toContainText("relay-a");
  await page.evaluate(async () => {
    const generatedPath = "/src/generated/management-client.ts";
    const queryPath = "/src/api/queryClient.ts";
    const { ManagementApi } = await import(generatedPath);
    const { queryClient } = await import(queryPath);
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async function (operation: string, request: unknown) {
      if (operation === "listUpstreams") {
        ManagementApi.prototype.request = original;
        return new Response(JSON.stringify({ error: { code: "management_lifecycle_unavailable", message: "Temporary read failure" } }), { status: 503 });
      }
      return original.call(this, operation, request);
    };
    await queryClient.invalidateQueries({ queryKey: ["upstreams"] });
  });
  await expect(page.getByRole("alert")).toContainText("读取失败");
  await expect(page.getByRole("alert")).toContainText("上次读取");
  await expect(page.locator("tbody")).toContainText("relay-a");
  await page.getByRole("button", { name: "重试读取" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page).not.toHaveURL(/unlock/u);
});
