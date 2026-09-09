import { expect, test } from "@playwright/test";
import { unlock, navigate, selectDraft } from "./helpers";

test("billing processing is visible across operational pages without requiring a version", async ({ page }) => {
  await unlock(page);
  for (const name of ["总览", "计费与价格", "用量分析", "请求与失败"]) {
    await navigate(page, name);
    const status = page.getByRole("complementary", { name: "计费处理状态" });
    await expect(status).toContainText("待修复");
    await expect(status).toContainText("即使水位追平");
    await status.getByText("处理水位与观测", { exact: true }).click();
    await expect(status).toContainText("246");
    await expect(status).toContainText("空账本不等于零消费");
  }
  await selectDraft(page);
  await expect(page.getByRole("complementary", { name: "计费处理状态" })).toContainText("待修复");
});
