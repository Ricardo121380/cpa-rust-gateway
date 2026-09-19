import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { CancelledError } from "@tanstack/react-query";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import type { ManagementOperationName, ManagementRequest } from "../../generated/management-client";

export type ProviderResourceReceipt = Readonly<{
  kind: "saved_draft" | "saved_applied" | "saved_unapplied" | "unconfirmed";
  workingVersion: ConfigVersionSummary;
  message: string;
}>;

type ProviderTask = Awaited<ReturnType<typeof beginConfigurationTask>>;
type ResourceWrite = <T>(operation: ManagementOperationName, request: ManagementRequest) => Promise<T>;

function isKnownUnpersistedWriteError(cause: unknown): boolean {
  const error = asAppError(cause);
  return error.kind === "invalid_request" || error.kind === "conflict" || error.kind === "session_invalid";
}

async function observeWorkingVersion(task: ProviderTask): Promise<ConfigVersionSummary> {
  task.assertOwner();
  const observed = await call<ConfigVersionSummary>("getConfigVersion", {
    path: { config_version_id: task.version.id },
  });
  task.assertOwner();
  return observed;
}

function isCancelled(cause: unknown): cause is CancelledError {
  return cause instanceof CancelledError;
}

async function unconfirmedWriteReceipt(task: ProviderTask, acknowledgedRevision: string, cause: unknown): Promise<ProviderResourceReceipt> {
  try {
    const observed = await observeWorkingVersion(task);
    return {
      kind: "unconfirmed",
      workingVersion: observed,
      message: observed.revision === acknowledgedRevision
        ? `修改请求的结果未确认：${asAppError(cause).message}。已重新读取工作配置；不要重复提交本次操作。`
        : `修改请求的结果未确认：${asAppError(cause).message}。工作配置已有后续变化，请不要重复提交本次操作。`,
    };
  } catch (reconciliationCause) {
    if (isCancelled(reconciliationCause)) throw reconciliationCause;
    return {
      kind: "unconfirmed",
      workingVersion: { ...task.version, revision: acknowledgedRevision },
      message: `修改请求的结果未确认：${asAppError(cause).message}。请核对工作配置；不要重复提交本次操作。`,
    };
  }
}

/**
 * A resource mutation and configuration application are two acknowledged
 * stages. `finish()` can fail after the resource write has persisted, so the
 * caller receives a non-replayable receipt instead of a fresh edit form.
 */
export async function runProviderResourceTask<T>(
  description: string,
  mutate: (task: ProviderTask, write: ResourceWrite) => Promise<T>,
): Promise<Readonly<{ value: T; receipt: ProviderResourceReceipt }>> {
  const task = await beginConfigurationTask(description);
  let writeStarted = false;
  let acknowledgedRevision = task.revision();
  const write: ResourceWrite = async <T,>(operation: ManagementOperationName, request: ManagementRequest): Promise<T> => {
    writeStarted = true;
    const value = await task.mutate(operation, request);
    acknowledgedRevision = task.revision();
    return value as T;
  };
  let value: T;
  try {
    value = await mutate(task, write);
  } catch (cause) {
    if (isCancelled(cause)) throw cause;
    if (!writeStarted || isKnownUnpersistedWriteError(cause)) throw cause;
    return {
      value: undefined as T,
      receipt: await unconfirmedWriteReceipt(task, acknowledgedRevision, cause),
    };
  }

  if (!task.autoApply) {
    return {
      value,
      receipt: {
        kind: "saved_draft",
        workingVersion: { ...task.version, revision: task.revision() },
        message: "已保存到当前草稿，尚未应用到运行服务。",
      },
    };
  }

  try {
    const applied = await task.finish();
    return {
      value,
      receipt: {
        kind: "saved_applied",
        workingVersion: applied,
        message: "已保存并应用到运行服务。",
      },
    };
  } catch (cause) {
    if (isCancelled(cause)) throw cause;
    try {
      const observed = await observeWorkingVersion(task);
      if (observed.status === "active" && observed.revision === acknowledgedRevision) {
        return {
          value,
          receipt: {
            kind: "saved_applied",
            workingVersion: observed,
            message: "修改已保存并已应用；应用回执需要重新核对。",
          },
        };
      }
      if (observed.status === "draft" && observed.revision === acknowledgedRevision) {
        return {
          value,
          receipt: {
            kind: "saved_unapplied",
            workingVersion: observed,
            message: "修改已保存，但应用尚未完成。请核对工作配置；不要重复提交本次操作。",
          },
        };
      }
      return {
        value,
        receipt: {
          kind: "unconfirmed",
          workingVersion: observed,
          message: "修改已保存，但工作配置已有后续变化；应用结果未确认。请核对工作配置；不要重复提交本次操作。",
        },
      };
    } catch (reconciliationCause) {
      if (isCancelled(reconciliationCause)) throw reconciliationCause;
      return {
        value,
        receipt: {
          kind: "unconfirmed",
          workingVersion: { ...task.version, revision: acknowledgedRevision },
          message: `修改可能已保存，但应用结果未确认：${asAppError(cause).message}`,
        },
      };
    }
  }
}
