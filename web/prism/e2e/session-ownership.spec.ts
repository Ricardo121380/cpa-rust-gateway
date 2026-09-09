import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("an auth denial clears the live UI, polling cache and reveal-once state", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
  await page.getByRole("button", { name: "签发 Client Key" }).click();
  await page.getByRole("dialog").getByLabel("Key ID").fill("session-expiry-test");
  await page.getByRole("dialog").getByRole("button", { name: "签发", exact: true }).click();
  await expect(page.locator(".reveal-key")).toBeVisible();

  // A single synthetic server rejection through the actual API boundary.
  // This is a fixture integration test; real listener acceptance is separate.
  await page.evaluate(async () => {
    const generatedPath = "/src/generated/management-client.ts";
    const clientPath = "/src/api/client.ts";
    const { ManagementApi } = await import(generatedPath);
    const { call } = await import(clientPath);
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async () => new Response(
      JSON.stringify({ error: { code: "management_access_denied" } }), { status: 404 },
    );
    try { await call("listConfigVersions"); } catch { /* expected auth denial */ }
    finally { ManagementApi.prototype.request = original; }
  });
  await expect(page).toHaveURL(/#\/unlock$/u);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator(".reveal-key")).toHaveCount(0);
  const state = await page.evaluate(async () => {
    const sessionPath = "/src/session/sessionStore.ts";
    const queryPath = "/src/api/queryClient.ts";
    const { useSessionStore } = await import(sessionPath);
    const { queryClient } = await import(queryPath);
    const session = useSessionStore.getState();
    return { unlocked: session.unlocked, hasKey: session.managementKey !== undefined,
      hasCsrf: session.csrfToken !== undefined, queries: queryClient.getQueryCache().getAll().length,
      mutations: queryClient.getMutationCache().getAll().length };
  });
  expect(state).toEqual({ unlocked: false, hasKey: false, hasCsrf: false, queries: 0, mutations: 0 });
  await unlock(page);
  await expect(page.locator(".version-picker select")).toHaveValue("");
  await expect(page.locator(".reveal-key")).toHaveCount(0);
});

test("changing versions closes a form authored for the previous version", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "访问控制");
  await page.getByRole("button", { name: "签发 Client Key" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  // The modal intentionally blocks the picker; simulate a context update from
  // another application action, rather than bypassing modal pointer trapping.
  await page.evaluate(async () => {
    const path = "/src/features/config-versions/versionStore.ts";
    const { useVersionStore } = await import(path);
    useVersionStore.getState().select({ id: "v-2026-07", status: "active", revision: "rev-1", created_at_ms: 0, description: "" });
  });
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "签发 Client Key" })).toBeDisabled();
});
