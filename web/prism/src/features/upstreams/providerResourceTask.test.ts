import { beforeEach, describe, expect, it, vi } from "vitest";
import { CancelledError } from "@tanstack/react-query";

vi.mock("../../api/client", () => ({ call: vi.fn() }));
vi.mock("../config-versions/configurationTask", () => ({ beginConfigurationTask: vi.fn() }));

import { call } from "../../api/client";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { runProviderResourceTask } from "./providerResourceTask";

const version = { id: "draft", revision: "rev-4", status: "draft" as const, created_at_ms: 1, description: "" };

describe("provider resource task receipts", () => {
  beforeEach(() => vi.resetAllMocks());

  it("keeps an explicit draft write as saved without attempting publication", async () => {
    const task = { version, autoApply: false, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn() };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.receipt.kind).toBe("saved_draft");
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("does not offer a replay when application fails after the resource write", async () => {
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn().mockRejectedValue(new Error("publish unavailable")) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue({ ...version, revision: "rev-5", status: "draft" } as never);
    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.value).toBe("saved");
    expect(result.receipt.kind).toBe("saved_unapplied");
  });

  it("records an acknowledged active-context application separately from the resource write", async () => {
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn().mockResolvedValue({ ...version, status: "active", revision: "rev-6" }) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.receipt).toMatchObject({ kind: "saved_applied", workingVersion: { revision: "rev-6" } });
  });

  it("recognizes a durable active version after a lost application response", async () => {
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn().mockRejectedValue(new Error("response lost")) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue({ ...version, revision: "rev-5", status: "active" } as never);
    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.receipt.kind).toBe("saved_applied");
  });

  it("keeps a write unconfirmed when neither application nor reread can be acknowledged", async () => {
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn().mockRejectedValue(new Error("response lost")) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockRejectedValue(new Error("read unavailable"));
    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.receipt.kind).toBe("unconfirmed");
  });

  it("does not restore a commit action after an uncertain resource-write response", async () => {
    const task = {
      version,
      autoApply: true,
      revision: () => "rev-5",
      assertOwner: vi.fn(),
      mutate: vi.fn().mockRejectedValue(new Error("write response lost")),
      finish: vi.fn(),
    };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue({ ...version, revision: "rev-5", status: "draft" } as never);

    const result = await runProviderResourceTask("test", async (_task, write) =>
      write("createEndpoint", { path: { upstream_id: "provider-a" }, body: { id: "endpoint-a" } }),
    );

    expect(task.mutate).toHaveBeenCalledTimes(1);
    expect(task.finish).not.toHaveBeenCalled();
    expect(result.receipt).toMatchObject({ kind: "unconfirmed", workingVersion: { revision: "rev-5" } });
  });

  it("keeps a known validation or conflict rejection in the editable form", async () => {
    const conflict = { kind: "conflict", code: "management_revision_conflict", message: "stale", status: 409 };
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), mutate: vi.fn().mockRejectedValue(conflict), finish: vi.fn() };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);

    await expect(runProviderResourceTask("test", async (_task, write) =>
      write("createEndpoint", { path: { upstream_id: "provider-a" }, body: { id: "endpoint-a" } }),
    )).rejects.toBe(conflict);
    expect(task.finish).not.toHaveBeenCalled();
  });

  it("does not claim a recovered publication when another revision became active", async () => {
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(), finish: vi.fn().mockRejectedValue(new Error("response lost")) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);
    vi.mocked(call).mockResolvedValue({ ...version, revision: "rev-6", status: "active" } as never);

    const result = await runProviderResourceTask("test", async () => "saved");
    expect(result.receipt.kind).toBe("unconfirmed");
  });

  it("does not reconcile a cancelled task under a replacement owner", async () => {
    const cancellation = new CancelledError({ silent: true });
    const task = { version, autoApply: true, revision: () => "rev-5", assertOwner: vi.fn(() => { throw cancellation; }), finish: vi.fn().mockRejectedValue(new Error("response lost")) };
    vi.mocked(beginConfigurationTask).mockResolvedValue(task as never);

    await expect(runProviderResourceTask("test", async () => "saved")).rejects.toBe(cancellation);
    expect(call).not.toHaveBeenCalled();
  });
});
