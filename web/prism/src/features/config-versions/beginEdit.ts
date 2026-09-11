import { call } from "../../api/client";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

/** Prepare a real editable graph without exporting credentials into the browser. */
export async function beginConfigurationEdit(description: string, reuseCurrentDraft = true): Promise<ConfigVersionSummary> {
  const selected = useVersionStore.getState().context;
  const versions = await call<ConfigVersionSummary[]>("listConfigVersions");
  const draft = versions.find((version) => version.id === selected?.configVersionId && version.status === "draft");
  if (reuseCurrentDraft && draft) return draft;
  const active = versions.find((version) => version.status === "active");
  const id = `edit-${crypto.randomUUID()}`;
  if (!active) return call<ConfigVersionSummary>("createConfigVersion", {body: {id, parent_id: null, description}});
  return call<ConfigVersionSummary>("forkConfigVersion", {
    path: {config_version_id: active.id},
    headers: {"X-Config-Version": active.id, "If-Match": active.revision},
    body: {id, description},
  });
}
