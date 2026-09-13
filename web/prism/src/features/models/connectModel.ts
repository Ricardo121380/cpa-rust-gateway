import type { beginConfigurationTask } from "../config-versions/configurationTask";
import type { PublicModel, CandidateRecord, RouteListItem, RoutingPage, AliasRecord } from "./model";

export type ConfigurationTask = Awaited<ReturnType<typeof beginConfigurationTask>>;
export type ModelConnection = Readonly<{ upstreamModel: string; alias?: string; targetModelId?: string; endpointId: string; allowUnlisted: boolean }>;

export function parseModelIds(value: string): string[] {
  const models = [...new Set(value.split(/\r?\n/u).map((id) => id.trim()).filter(Boolean))];
  if (!models.length || models.length > 20 || models.some((id) => id.length > 256)) {
    throw new Error("每次填写 1–20 个模型 ID，每行一个，单个最长 256 字符。");
  }
  return models;
}

export async function readRoutingRows<T>(task: ConfigurationTask, operation: "listRoutes" | "listRouteCandidates" | "listModelAliases") {
  const rows: T[] = [];
  let cursor: string | undefined;
  let revision: string | undefined;
  do {
    const page = await task.read<RoutingPage<T>>(operation, { query: { limit: 100, ...(cursor ? { cursor } : {}) } });
    if (revision && revision !== page.revision) throw new Error("模型连接已变化，请重新读取。");
    revision = page.revision;
    rows.push(...page.items);
    cursor = page.next_cursor ?? undefined;
    if (rows.length >= 10000 && cursor) throw new Error("模型连接过多，请在高级配置中按路由维护。");
  } while (cursor);
  return rows;
}

/** One exact model can have many interfaces; repeated imports retain the existing connection. */
export async function connectModel(task: ConfigurationTask, input: ModelConnection) {
  const model = input.upstreamModel;
  const models = await task.read<PublicModel[]>("listPublicModels");
  const existing = models.find((row) => input.targetModelId ? row.id === input.targetModelId : row.model_name === model);
  if (input.targetModelId && !existing) throw new Error("所选模型已不存在，请重新读取。");
  const id = existing?.id ?? `model-${crypto.randomUUID()}`;
  const alias = input.alias?.trim();
  let needsAlias = false;
  if (alias && alias !== model && alias !== existing?.model_name) {
    if (models.some(row=>row.model_name===alias)) throw new Error("此别名已被其他模型使用。");
    const assigned=(await readRoutingRows<AliasRecord>(task,"listModelAliases")).find(row=>row.alias===alias);
    if(assigned && assigned.public_model_id!==id)throw new Error("此别名已被其他模型使用。");
    needsAlias=!assigned;
  }
  const addAlias=async()=>{if(needsAlias)await task.mutate("createModelAlias",{path:{public_model_id:id},body:{alias}});};
  let route: string | undefined;
  if (existing) {
    const routes = (await readRoutingRows<RouteListItem>(task, "listRoutes")).filter((row) => row.public_model_id === existing.id);
    if (routes.length > 1) throw new Error("模型包含多份路由，请在高级配置中核对。");
    route = routes[0]?.id;
    if (route) {
      const candidates = (await readRoutingRows<CandidateRecord>(task, "listRouteCandidates")).filter((row) => row.route_id === route);
      if (candidates.some((row) => row.upstream_model !== input.upstreamModel)) throw new Error("此名称已有自定义模型映射，请在模型详情中核对后添加连接。");
      if (candidates.some((row) => row.endpoint_id === input.endpointId && row.upstream_model === input.upstreamModel)) {
        await addAlias();
        return { id: existing.id, route, added: false };
      }
    }
  } else {
    await task.mutate("createPublicModel", { body: { id, model_name: model, display_name: model, status: "active", capabilities: {} } });
  }
  if (!route) {
    route = `route-${crypto.randomUUID()}`;
    await task.mutate("createRoute", { path: { public_model_id: id }, body: { id: route, policy: "smooth_weighted_round_robin", max_attempts: 1, bootstrap_timeout_ms: 30000 } });
  }
  await task.mutate("createRouteCandidate", { path: { route_id: route }, body: {
    id: `candidate-${crypto.randomUUID()}`, endpoint_id: input.endpointId, upstream_model: input.upstreamModel,
    credential_scope: "all_active", transform_mode: "canonical_bridge", enabled: true, priority: 0, weight: 1,
    capability_override: input.allowUnlisted ? { allow_unlisted_model: true } : {},
  } });
  await addAlias();
  return { id, route, added: true };
}
