import { call } from "../../api/client";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";
import { CancelledError } from "@tanstack/react-query";

/** Prepare a real editable graph without exporting credentials into the browser. */
export async function beginConfigurationEdit(description: string, reuseCurrentDraft = true): Promise<ConfigVersionSummary> {
  const owner = useVersionStore.getState();
  const selected = owner.context;
  const versions = await call<ConfigVersionSummary[]>("listConfigVersions");
  if(owner.selectionGeneration!==useVersionStore.getState().selectionGeneration)throw new CancelledError({silent:true});
  const draft = versions.find((version) => version.id === selected?.configVersionId && version.status === "draft");
  if (reuseCurrentDraft && draft) return draft;
  const active = versions.find((version) => version.status === "active");
  if(selected?.status==="active"&&(active?.id!==selected.configVersionId||active.revision!==selected.revision))throw new Error("当前配置已变化，请刷新页面后重新操作。");
  const id = `edit-${crypto.randomUUID()}`;
  if (!active) return call<ConfigVersionSummary>("createConfigVersion", {body: {id, parent_id: null, description}});
  return call<ConfigVersionSummary>("forkConfigVersion", {
    path: {config_version_id: active.id},
    headers: {"X-Config-Version": active.id, "If-Match": active.revision},
    body: {id, description},
  });
}
