// Error mapping for the management API. Every error body is {error:{code,message}} (contract schema "Error").

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string, public retryAfter?: number) {
    super(message);
  }
}

/** User-facing explanations keyed by backend error code; unknown codes fall back to the server message. */
const COPY: Record<string, string> = {
  management_access_denied: "管理接口拒绝了这次访问：凭据无效、会话已过期，或页面不是从网关管理地址打开的。",
  management_login_invalid_credentials: "用户名或密码不正确。",
  management_login_invalid_password: "新密码需要 12–128 个字符。",
  management_login_invalid_request: "登录请求格式不正确。",
  management_login_rate_limited: "尝试次数过多，请稍后再试。",
  management_login_unavailable: "网关没有启用管理员登录。可以改用管理 Key 登录。",
  management_revision_conflict: "配置已被其他人修改。已刷新到最新版本，请检查后重试。",
  management_version_not_writable: "只能修改草稿。先从当前版本创建草稿，再继续编辑。",
  management_credential_revision_conflict: "凭据已被修改，请刷新后重试。",
  management_operations_cursor_conflict: "数据在翻页期间发生了变化，请从第一页重新加载。",
  management_provider_account_pool_cursor_conflict: "账号池在翻页期间发生了变化，请从第一页重新加载。",
  management_provider_account_action_target_changed: "目标账号的状态已经变化，操作没有执行。请刷新后确认。",
  management_billing_catalog_conflict: "价格目录已被修改，请刷新后重试。",
  management_resource_not_found: "找不到这个资源，它可能已被删除。",
  management_operations_busy: "运营数据正在处理，请稍后再试。",
  management_runtime_unavailable: "这个功能在当前网关部署中没有启用。",
  management_internal_error: "网关内部错误。请查看网关日志。",
  invalid_management_request: "请求缺少必要的参数或请求头。",
};

export function describeError(e: unknown): { title: string; detail?: string } {
  if (e instanceof ApiError) {
    const title = COPY[e.code] ?? (e.message || `请求失败（HTTP ${e.status}）`);
    return { title, detail: `${e.code} · HTTP ${e.status}` };
  }
  if (e instanceof TypeError) return { title: "无法连接到网关管理接口。", detail: e.message };
  return { title: e instanceof Error ? e.message : String(e) };
}

export const isUnavailable = (e: unknown) => e instanceof ApiError && (e.status === 503 || e.code === "management_runtime_unavailable");
export const isConflict = (e: unknown) => e instanceof ApiError && e.status === 409;
export const isDenied = (e: unknown) => e instanceof ApiError && (e.status === 401 || e.code === "management_access_denied");
