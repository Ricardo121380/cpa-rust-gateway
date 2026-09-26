import { describe, expect, it } from "vitest";
import { classifyStatus, requiresWriteReconciliation, isRuntimeConflict, shouldRetryManagementRead, toAppError, type AppError } from "./errors";

function conflict(code: string): AppError {
  return { kind: "conflict", code, message: "", status: 409 };
}

describe("classifyStatus", () => {
  it("separates a dead session from an ordinary 404", () => {
    expect(classifyStatus(404, "management_access_denied")).toBe("session_invalid");
    expect(classifyStatus(404, "management_resource_not_found")).toBe("unknown");
  });
});

describe("toAppError", () => {
  it("normalizes the legacy OAuth string error envelope", async () => {
    await expect(toAppError(new Response(JSON.stringify({ error: "oauth_callback_rejected" }), { status: 409 }))).resolves.toMatchObject({
      kind: "conflict",
      code: "oauth_callback_rejected",
    });
  });
});

describe("isRuntimeConflict", () => {
  it("keeps the config banner off a runtime snapshot rotating", () => {
    // The shell's conflict bar reads "配置已被其他会话修改". For these five
    // codes nobody edited anything — an operational cursor went stale, or an
    // action's target moved between the read and the write. Raising the banner
    // sends the operator hunting a config change that never happened.
    expect(isRuntimeConflict(conflict("management_inventory_cursor_conflict"))).toBe(true);
    expect(isRuntimeConflict(conflict("management_operations_cursor_conflict"))).toBe(true);
    expect(isRuntimeConflict(conflict("management_provider_account_pool_cursor_conflict"))).toBe(
      true,
    );
    expect(isRuntimeConflict(conflict("management_provider_egress_status_cursor_conflict"))).toBe(
      true,
    );
    expect(isRuntimeConflict(conflict("management_provider_account_action_target_changed"))).toBe(
      true,
    );
    expect(isRuntimeConflict(conflict("management_channel_pin_target_changed"))).toBe(true);
  });

  it("still raises it when the configuration really did move", () => {
    expect(isRuntimeConflict(conflict("management_revision_conflict"))).toBe(false);
    expect(isRuntimeConflict(conflict("management_credential_revision_conflict"))).toBe(false);
    expect(isRuntimeConflict(conflict("management_billing_catalog_conflict"))).toBe(false);
    // This one names the config explicitly: the selected version is no longer
    // the snapshot's source, so the banner is the correct response.
    expect(isRuntimeConflict(conflict("management_provider_egress_status_config_conflict"))).toBe(
      false,
    );
  });

  it("is a 409-only question", () => {
    expect(
      isRuntimeConflict({
        kind: "unavailable",
        code: "management_operations_cursor_conflict",
        message: "",
        status: 503,
      }),
    ).toBe(false);
  });
});

describe("shouldRetryManagementRead", () => {
  const unavailable = { kind: "unavailable", code: "management_operations_busy", message: "busy", status: 503 } as const;
  it("retries only bounded unavailable reads", () => {
    expect(shouldRetryManagementRead(0, unavailable)).toBe(true);
    expect(shouldRetryManagementRead(2, unavailable)).toBe(true);
    expect(shouldRetryManagementRead(3, unavailable)).toBe(false);
    expect(shouldRetryManagementRead(0, { ...unavailable, kind: "conflict" })).toBe(false);
  });
});

it("401 expires a session and ambiguous writes require reconciliation",()=>{
  expect(classifyStatus(401,"unknown")).toBe("session_invalid");
  expect(isRuntimeConflict(conflict("management_account_inventory_conflict"))).toBe(true);
  expect(isRuntimeConflict(conflict("management_request_cursor_conflict"))).toBe(true);
  for(const kind of ["network","conflict","unavailable","unknown"])expect(requiresWriteReconciliation({kind})).toBe(true);
  expect(requiresWriteReconciliation({kind:"invalid_request"})).toBe(false);
});
