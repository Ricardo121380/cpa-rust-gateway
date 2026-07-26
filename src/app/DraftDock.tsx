// Draft dock — the third (and last) chrome glass pane. Appears only when the
// selected version is a draft; publish success re-selects the same version,
// whose status flip drives the topbar's anneal transition (material CSS).
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../api/client";
import { asAppError } from "../api/errors";
import { GlassSurface } from "../components/glass/GlassSurface";
import { Sheet } from "../components/Sheet";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../features/config-versions/versionStore";

type Validation = Readonly<{ valid: boolean; error_codes?: readonly string[] }>;
type Publication = Readonly<{
  active_config_version_id: string;
  replaced_config_version_id?: string | null;
}>;

export function DraftDock() {
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const select = useVersionStore((s) => s.select);
  const [validation, setValidation] = useState<Validation | undefined>();
  const [publication, setPublication] = useState<Publication | undefined>();
  const [error, setError] = useState<string | undefined>();

  const validate = useMutation({
    mutationFn: (id: string) =>
      call<Validation>("validateConfigVersion", { path: { config_version_id: id } }),
    onSuccess: setValidation,
    onError: (cause) => setError(asAppError(cause).message),
  });

  const publish = useMutation({
    mutationFn: (id: string) =>
      call<Publication>("publishConfigVersion", { path: { config_version_id: id } }, { mutating: true }),
    onSuccess: async (result) => {
      setPublication(result);
      await queryClient.invalidateQueries({ queryKey: ["config-versions"] });
      // Re-select the same version: its status flipped draft → active, which
      // triggers the anneal transition on every material-bound glass pane.
      const versions = await call<ConfigVersionSummary[]>("listConfigVersions");
      const published = versions.find((row) => row.id === result.active_config_version_id);
      if (published !== undefined) {
        select(published);
      }
    },
    onError: (cause) => setError(asAppError(cause).message),
  });

  if (context === undefined || context.status !== "draft") {
    return null;
  }

  return (
    <>
      <GlassSurface as="footer" className="dock" material="draft">
        <span>
          草稿 <span className="idchip mono">{context.configVersionId}</span>
          <span className="idchip mono">{context.revision}</span>
        </span>
        {error !== undefined ? <span className="dock-error">{error}</span> : null}
        <span className="dock-actions">
          <button
            type="button"
            className="secondary"
            disabled={validate.isPending}
            onClick={() => {
              setError(undefined);
              validate.mutate(context.configVersionId);
            }}
          >
            验证
          </button>
          <button
            type="button"
            disabled={publish.isPending}
            onClick={() => {
              setError(undefined);
              publish.mutate(context.configVersionId);
            }}
          >
            发布
          </button>
        </span>
      </GlassSurface>

      {validation !== undefined ? (
        <Sheet title="验证结果" onEscape={() => setValidation(undefined)}>
          {validation.valid ? (
            <p>通过 —— 可以发布。</p>
          ) : (
            <ul>
              {(validation.error_codes ?? []).map((code) => (
                <li key={code} className="mono">
                  {code}
                </li>
              ))}
            </ul>
          )}
          <div className="sheet-actions">
            <button type="button" onClick={() => setValidation(undefined)}>
              关闭
            </button>
          </div>
        </Sheet>
      ) : null}

      {publication !== undefined ? (
        <Sheet title="已发布(材质退火完成)">
          <p>
            活动版本:<span className="mono">{publication.active_config_version_id}</span>
          </p>
          {publication.replaced_config_version_id != null ? (
            <p>
              被替换:<span className="mono">{publication.replaced_config_version_id}</span>
              (保留为一步回滚目标)
            </p>
          ) : null}
          <div className="sheet-actions">
            <button type="button" onClick={() => setPublication(undefined)}>
              完成
            </button>
          </div>
        </Sheet>
      ) : null}
    </>
  );
}
