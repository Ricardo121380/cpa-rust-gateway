// Minimal message module; zh-CN default, en added in FE-5.
export const messages = {
  appTitle: "Prism · 网关管理",
  unlock: {
    title: "解锁管理面板",
    managementKey: "Management Key",
    csrfToken: "CSRF Token(浏览器部署必填)",
    submit: "解锁",
    hint: "密钥仅存于本页内存,刷新后需要重新输入。",
    failed: "管理访问不可用 —— 请检查密钥、网络位置与部署配置。",
    invalidShape: "格式不符:Management Key 需为 mgmt_ 前缀,32-512 位字母数字或 _ -。",
  },
  nav: {
    overview: "总览",
    usage: "用量分析",
    monitoring: "请求监控",
    versions: "配置版本",
    upstreams: "上游",
    models: "模型与路由",
    access: "访问控制",
    egress: "出口策略",
    runtime: "运行时",
    audit: "审计与备份",
  },
  version: {
    none: "未选择版本",
    conflict: "配置已被其他会话修改,已刷新数据 —— 请确认后重试。",
    readOnly: "当前版本只读(非草稿)。",
  },
  state: {
    empty: "暂无数据",
    filteredEmpty: "没有符合过滤条件的结果",
    unavailable: "此部署未启用该运行时投影",
    unwired: "事件管道尚未接线(G2)—— 观测数据在后端接线后出现",
  },
} as const;
