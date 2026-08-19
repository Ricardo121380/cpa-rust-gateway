// Request monitor: JSONL export. The usage tests that shared this file went
// with the analytics shape they were written for — see e2e/usage.spec.ts.
//
// The failure mode here is not visible to a unit test: a download the
// production CSP blocks, or a file that is not parseable line by line.
import { expect, test } from "@playwright/test";
import { navigate, unlock } from "./helpers";

test("monitoring exports parseable JSONL with no request bodies", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");

  const button = page.getByRole("button", { name: /导出 JSONL/u });
  await expect(button).toBeEnabled();
  // the label states the row count and whether the export is partial
  await expect(button).toContainText(/\d+ 行/u);

  const download = await Promise.all([page.waitForEvent("download"), button.click()]).then(
    ([event]) => event,
  );
  expect(download.suggestedFilename()).toMatch(/^prism-requests-[\d-]+(-partial)?\.jsonl$/u);

  const stream = await download.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString("utf8");
  const lines = text.trimEnd().split("\n");

  expect(lines.length).toBeGreaterThan(1);
  for (const line of lines) {
    expect(() => JSON.parse(line) as unknown).not.toThrow();
  }
  const header = JSON.parse(lines[0]!) as Record<string, unknown>;
  expect(header["format"]).toBe("prism.requests.v1");
  expect(header).toHaveProperty("window");
  // value-free: no body ever reaches the file, because none exists upstream
  expect(text).not.toMatch(/"(body|request_body|prompt|messages|content)"/u);
});

test("import is absent and says why, rather than being a dead control", async ({ page }) => {
  await unlock(page);
  await navigate(page, "请求监控");
  await expect(page.locator(".export-note")).toContainText("G3");
  await expect(page.getByRole("button", { name: /导入/u })).toHaveCount(0);
});
