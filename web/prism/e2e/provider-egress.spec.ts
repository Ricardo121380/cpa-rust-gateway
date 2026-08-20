// Provider 出口状态 · 三个域 (P13-11E4) on the runtime page.
//
// The assertions target the boundaries the backend handed over with the
// projection: three domains stay three, nothing is aggregated into a health
// value, nothing is actionable, and an empty domain is never read as a
// healthy one.
import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

const CARD = ".rt-card:has-text('Provider 出口状态')";

test("the three domains stay three tables, never one", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  // Each partition is its own section with its own head and its own snapshot —
  // three reads, so the snapshots can legitimately differ.
  await expect(page.locator('.rt-domain[data-domain="egress"]')).toBeVisible();
  await expect(page.locator('.rt-domain[data-domain="session"]')).toBeVisible();
  await expect(page.locator('.rt-domain[data-domain="clearance"]')).toBeVisible();

  const card = page.locator(CARD);
  await expect(card).toContainText("三个独立的域");
  await expect(card).toContainText("不合成任何 overall health");
  await expect(card).toContainText("快照可能不同");

  // Read-only projection: the contract has no recover/refresh operation on it,
  // so the card must not grow one. Paging and re-reading are not actions.
  await expect(card.getByRole("button", { name: "冷却" })).toHaveCount(0);
  await expect(card.getByRole("button", { name: /恢复/u })).toHaveCount(0);
  await expect(card.getByRole("button", { name: /刷新凭证|重置|探测/u })).toHaveCount(0);
});

test("an empty domain says the source does not exist, not that it is healthy", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  // clearance is empty on purpose: the projection's source only covers
  // assembled Grok Build/Console state, so production Web/clearance can be
  // truthfully empty. That is the exact sentence P13-11E4 required on screen.
  const clearance = page.locator('.rt-domain[data-domain="clearance"]');
  await expect(clearance.locator(".empty-state")).toContainText("该来源不存在");
  await expect(clearance.locator(".empty-state")).toContainText(
    "不等于健康、可用、新鲜、已测试或可用于生产",
  );
  await expect(clearance.locator("tbody tr")).toHaveCount(0);
});

test("a named target with no id is not rendered as a direct one", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const egress = page.locator('.rt-domain[data-domain="egress"]');
  await expect(egress.locator("tr", { hasText: "ep-relay-a-responses" })).toContainText("直连");
  // target_kind and target_id are independently nullable. A blank cell would
  // erase the difference between "went direct" and "named, id not reported".
  await expect(egress.locator("tr", { hasText: "ep-grok-console" })).toContainText("未报告");
  await expect(egress.locator("tr", { hasText: "ep-grok-console" })).not.toContainText("直连");
});

test("each domain's chips come from that domain's vocabulary only", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const egress = page.locator('.rt-domain[data-domain="egress"]');
  await expect(egress.locator('.rt-chip[data-state="probe_due"]')).toBeVisible();
  await expect(egress.locator('.rt-chip[data-state="circuit_open"]')).toBeVisible();
  // `fresh` and `active` belong to the other two domains; if a shared lookup
  // table ever creeps in, they would start rendering here.
  await expect(egress.locator('.rt-chip[data-state="fresh"]')).toHaveCount(0);
  await expect(egress.locator('.rt-chip[data-state="active"]')).toHaveCount(0);

  const session = page.locator('.rt-domain[data-domain="session"]');
  await expect(session.locator('.rt-chip[data-state="active"]').first()).toBeVisible();
  await expect(session.locator('.rt-chip[data-state="probe_due"]')).toHaveCount(0);
});

test("a rotated snapshot stops paging and restarts from the first page", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const session = page.locator('.rt-domain[data-domain="session"]');
  await expect(session.locator("tbody tr")).toHaveCount(100);

  // Page two: the opaque cursor is passed back exactly as received.
  await session.getByRole("button", { name: "继续读取" }).click();
  await expect(session.locator("tbody tr")).toHaveCount(200);

  // Page three: the runtime snapshot rotated underneath. Continuing would
  // splice two snapshots into one list that never existed.
  await session.getByRole("button", { name: "继续读取" }).click();
  await expect(session.locator(".action-error")).toContainText("快照已轮换");
  // The rows already read stay on screen — they were true when they were read
  // — and the recovery is explicit rather than a silent swap.
  await expect(session.locator("tbody tr")).toHaveCount(200);

  await session.getByRole("button", { name: "从头重读" }).click();
  await expect(session.locator("tbody tr")).toHaveCount(100);
  await expect(session.locator(".action-error")).toHaveCount(0);
});

test("a rotated snapshot does not claim the configuration changed", async ({ page }) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page, "运行时");

  const session = page.locator('.rt-domain[data-domain="session"]');
  await session.getByRole("button", { name: "继续读取" }).click();
  await expect(session.locator("tbody tr")).toHaveCount(200);
  await session.getByRole("button", { name: "继续读取" }).click();
  await expect(session.locator(".action-error")).toBeVisible();

  // The shell's conflict bar means "someone edited your config". A runtime
  // snapshot rotating is not that, and firing the bar would send the operator
  // looking for a change nobody made.
  await expect(page.locator(".conflict-bar")).toHaveCount(0);
});
